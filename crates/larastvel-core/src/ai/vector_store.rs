//! Vector stores — the Rust equivalent of Laravel 13's `VectorStore` from
//! `laravel/ai`.
//!
//! A vector store ingests documents, embeds them, and answers similarity
//! queries. Two implementations ship with the framework:
//!
//! - [`FileVectorStore`] — a local JSON-file store, no database needed.
//! - [`PostgresVectorStore`] — PostgreSQL + `pgvector` (the Laravel default
//!   backing store); use `VectorSimilarityQuery` for ad-hoc semantic search
//!   on your own tables.

use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::messages::EmbeddingOptions;
use super::provider::ProviderError;
use super::Ai;

/// The default directory / file used by [`Ai::vector_store`].
pub const DEFAULT_VECTOR_STORE_PATH: &str = "storage/ai/vector-store.json";

/// An error raised by a vector store.
#[derive(Debug, thiserror::Error)]
pub enum VectorStoreError {
    #[error("vector store I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("vector store embedding error: {0}")]
    Embedding(#[from] ProviderError),
    #[error("vector store database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("vector store JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// One retrieved document, mirroring the Laravel AI SDK's vector query
/// items (`id`, `content`, `additionalArguments`).
#[derive(Debug, Clone, PartialEq)]
pub struct VectorQueryItem {
    /// The document's path / id.
    pub id: String,
    /// The document's content.
    pub content: String,
    /// Provider-specific extra metadata.
    pub additional_arguments: Value,
}

/// The result of a vector query, mirroring Laravel's `VectorQueryResult`.
#[derive(Debug, Clone, Default)]
pub struct VectorQueryResult {
    /// Matching documents, best first.
    pub items: Vec<VectorQueryItem>,
}

impl VectorQueryResult {
    /// The best matching item, if any.
    pub fn first(&self) -> Option<&VectorQueryItem> {
        self.items.first()
    }
}

/// A document store that answers similarity queries via embeddings —
/// Laravel 13's `VectorStore` contract (`addFileContent`, `addFile`,
/// `query`, `delete`).
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync + std::fmt::Debug {
    /// Embed and store a document at `path`.
    async fn add_file_content(&self, path: &str, content: &str) -> Result<(), VectorStoreError>;

    /// Embed and store a document at `path`.
    async fn add_file(&self, path: &str, content: &str) -> Result<(), VectorStoreError> {
        self.add_file_content(path, content).await
    }

    /// Find the `top_k` documents closest to the query.
    async fn query(&self, query: &str, top_k: usize)
        -> Result<VectorQueryResult, VectorStoreError>;

    /// Remove all documents stored under `path`.
    async fn delete(&self, path: &str) -> Result<(), VectorStoreError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileRecord {
    path: String,
    content: String,
    embedding: Vec<f32>,
    additional_arguments: Value,
}

/// A vector store persisted to a local JSON file. Embeddings go through the
/// given [`Ai`] (including its embedding cache), so repeated content never
/// re-embeds. Querying embeds the query and scans the file's records with
/// cosine similarity.
#[derive(Debug)]
pub struct FileVectorStore {
    ai: Arc<Ai>,
    path: Arc<str>,
    records: Mutex<Vec<FileRecord>>,
    embedding_model: Option<String>,
}

impl FileVectorStore {
    /// Create a store backed by the JSON file at `path` (created on first
    /// write; its parent directories are created too).
    pub async fn new(ai: Arc<Ai>, path: impl Into<String>) -> Result<Self, VectorStoreError> {
        let path: Arc<str> = path.into().into();
        let records = Self::load(&path).await.unwrap_or_default();
        Ok(Self {
            ai,
            path,
            records: Mutex::new(records),
            embedding_model: None,
        })
    }

    /// Use a specific embedding model for this store's requests.
    pub fn with_embedding_model(mut self, model: impl Into<String>) -> Self {
        self.embedding_model = Some(model.into());
        self
    }

    /// The JSON file backing this store.
    pub fn path(&self) -> &str {
        &self.path
    }

    async fn load(path: &str) -> Result<Vec<FileRecord>, VectorStoreError> {
        let content = tokio::fs::read_to_string(path).await?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    }

    async fn save(&self, records: &[FileRecord]) -> Result<(), VectorStoreError> {
        if let Some(parent) = Path::new(&*self.path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let tmp = format!("{}.tmp", self.path);
        tokio::fs::write(&tmp, serde_json::to_string(records)?).await?;
        tokio::fs::rename(&tmp, &*self.path).await?;
        Ok(())
    }

    async fn embed(&self, input: &str) -> Result<Vec<f32>, VectorStoreError> {
        let options = EmbeddingOptions {
            model: self.embedding_model.clone(),
        };
        Ok(self.ai.provider().embed(input, &options).await?)
    }

    fn cosine(a: &[f32], b: &[f32]) -> f64 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        (dot / (norm_a * norm_b)) as f64
    }
}

#[async_trait::async_trait]
impl VectorStore for FileVectorStore {
    async fn add_file_content(&self, path: &str, content: &str) -> Result<(), VectorStoreError> {
        let embedding = self.embed(content).await?;
        let mut records = self.records.lock().await;
        records.retain(|record| record.path != path);
        records.push(FileRecord {
            path: path.to_string(),
            content: content.to_string(),
            embedding,
            additional_arguments: Value::Object(Default::default()),
        });
        let snapshot = records.clone();
        drop(records);
        self.save(&snapshot).await
    }

    async fn query(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<VectorQueryResult, VectorStoreError> {
        let embedding = self.embed(query).await?;
        let records = self.records.lock().await;
        let mut scored: Vec<(f64, &FileRecord)> = records
            .iter()
            .map(|record| (Self::cosine(&embedding, &record.embedding), record))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(VectorQueryResult {
            items: scored
                .into_iter()
                .take(top_k)
                .map(|(_, record)| VectorQueryItem {
                    id: record.path.clone(),
                    content: record.content.clone(),
                    additional_arguments: record.additional_arguments.clone(),
                })
                .collect(),
        })
    }

    async fn delete(&self, path: &str) -> Result<(), VectorStoreError> {
        let mut records = self.records.lock().await;
        let before = records.len();
        records.retain(|record| record.path != path);
        let removed = before != records.len();
        let snapshot = records.clone();
        drop(records);
        if removed {
            self.save(&snapshot).await?;
        }
        Ok(())
    }
}

/// Options for the PostgreSQL + `pgvector` store.
#[derive(Debug, Clone)]
pub struct PostgresVectorStoreOptions {
    /// The table storing the vectors. Defaults to `vector_store_items`.
    pub table: String,
    /// The embedding dimension. Defaults to 1536 (OpenAI
    /// `text-embedding-3-small`).
    pub embedding_dim: usize,
}

impl Default for PostgresVectorStoreOptions {
    fn default() -> Self {
        Self {
            table: "vector_store_items".into(),
            embedding_dim: 1536,
        }
    }
}

/// A vector store backed by PostgreSQL + `pgvector`, mirroring Laravel 13's
/// `PostgresVectorStore`.
///
/// The store manages a table (`vector_store_items` by default) with
/// `file_path`, `content`, `embedding vector(1536)`, and
/// `additional_arguments jsonb` columns; rows are matched with cosine
/// distance (`<=>`), the Laravel default.
#[derive(Debug)]
pub struct PostgresVectorStore {
    conn: sea_orm::DatabaseConnection,
    options: PostgresVectorStoreOptions,
    ai: Arc<Ai>,
}

impl PostgresVectorStore {
    /// Create a store for the given database connection. The backing table
    /// is created on demand when the store is first used.
    pub fn new(
        conn: sea_orm::DatabaseConnection,
        ai: Arc<Ai>,
        options: PostgresVectorStoreOptions,
    ) -> Self {
        Self { conn, options, ai }
    }

    /// Ensure the backing table exists (`CREATE TABLE IF NOT EXISTS` plus
    /// the pgvector extension when needed).
    pub async fn ensure_table(&self) -> Result<(), VectorStoreError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS \"{}\" (
                id BIGSERIAL PRIMARY KEY,
                file_path TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding vector({}) NOT NULL,
                additional_arguments JSONB DEFAULT '{{}}'::jsonb
            )",
            self.options.table, self.options.embedding_dim
        );
        self.conn.execute_unprepared(&sql).await?;
        Ok(())
    }

    async fn embed(&self, input: &str) -> Result<Vec<f32>, VectorStoreError> {
        Ok(self
            .ai
            .provider()
            .embed(input, &EmbeddingOptions::default())
            .await?)
    }

    fn vector_literal(embedding: &[f32]) -> String {
        let inner = embedding
            .iter()
            .map(f32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!("'[{inner}]'::vector")
    }

    fn query_sql(options: &PostgresVectorStoreOptions, top_k: usize) -> String {
        format!(
            "SELECT file_path, content, additional_arguments FROM \"{}\" \
             ORDER BY embedding <=> $1::vector ASC LIMIT {top_k}",
            options.table
        )
    }

    fn delete_sql(options: &PostgresVectorStoreOptions) -> String {
        format!("DELETE FROM \"{}\" WHERE file_path = $1", options.table)
    }
}

#[async_trait::async_trait]
impl VectorStore for PostgresVectorStore {
    async fn add_file_content(&self, path: &str, content: &str) -> Result<(), VectorStoreError> {
        let embedding = self.embed(content).await?;
        self.ensure_table().await?;
        let sql = format!(
            "INSERT INTO \"{}\" (file_path, content, embedding, additional_arguments) \
             VALUES ($1, $2, {}, '{{}}'::jsonb)",
            self.options.table,
            Self::vector_literal(&embedding)
        );
        let statement = sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            &sql,
            vec![path.into(), content.into()],
        );
        self.conn.execute(statement).await?;
        Ok(())
    }

    async fn query(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<VectorQueryResult, VectorStoreError> {
        let embedding = self.embed(query).await?;
        let sql = Self::query_sql(&self.options, top_k);
        let statement = sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            &sql,
            vec![Self::vector_literal(&embedding).into()],
        );
        let rows = self.conn.query_all(statement).await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.try_get("", "file_path")?;
            let content: String = row.try_get("", "content")?;
            let additional_arguments: String = row.try_get("", "additional_arguments")?;
            items.push(VectorQueryItem {
                id,
                content,
                additional_arguments: serde_json::from_str(&additional_arguments)
                    .unwrap_or(Value::Null),
            });
        }
        Ok(VectorQueryResult { items })
    }

    async fn delete(&self, path: &str) -> Result<(), VectorStoreError> {
        let sql = Self::delete_sql(&self.options);
        let statement = sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Postgres,
            &sql,
            vec![path.into()],
        );
        self.conn.execute(statement).await?;
        Ok(())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::FakeAi;

    fn fake_ai() -> Arc<Ai> {
        Arc::new(Ai::new(Arc::new(FakeAi::new())))
    }

    #[tokio::test]
    async fn test_file_store_add_query_delete() {
        let store = FileVectorStore::new(fake_ai(), "/tmp/larastvel-test-vector-store.json")
            .await
            .unwrap();
        store
            .add_file_content("docs/intro.md", "Larastvel is a Rust framework")
            .await
            .unwrap();
        store
            .add_file_content("docs/api.md", "Axum powers the routing")
            .await
            .unwrap();

        let result = store.query("routing framework", 2).await.unwrap();
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.first().unwrap().id, "docs/api.md");

        let top_1 = store.query("Larastvel framework", 1).await.unwrap();
        assert_eq!(top_1.items.len(), 1);

        store.delete("docs/api.md").await.unwrap();
        let result = store.query("routing framework", 2).await.unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.first().unwrap().id, "docs/intro.md");

        let _ = tokio::fs::remove_file("/tmp/larastvel-test-vector-store.json").await;
    }

    #[tokio::test]
    async fn test_file_store_replaces_existing_path() {
        let store = FileVectorStore::new(fake_ai(), "/tmp/larastvel-test-vector-store-2.json")
            .await
            .unwrap();
        store
            .add_file_content("docs/a.md", "first version")
            .await
            .unwrap();
        store
            .add_file_content("docs/a.md", "second version")
            .await
            .unwrap();

        let result = store.query("second", 1).await.unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.first().unwrap().content, "second version");

        let _ = tokio::fs::remove_file("/tmp/larastvel-test-vector-store-2.json").await;
    }

    #[test]
    fn test_vector_literal_format() {
        assert_eq!(
            PostgresVectorStore::vector_literal(&[0.1, 0.5, 1.0]),
            "'[0.1,0.5,1]'::vector"
        );
    }

    #[test]
    fn test_postgres_store_sql() {
        let sql = PostgresVectorStore::query_sql(&PostgresVectorStoreOptions::default(), 5);
        assert!(sql.contains("FROM \"vector_store_items\""));
        assert!(sql.contains("ORDER BY embedding <=> $1::vector ASC"));
        assert!(sql.contains("LIMIT 5"));

        let sql = PostgresVectorStore::delete_sql(&PostgresVectorStoreOptions::default());
        assert!(sql.contains("DELETE FROM \"vector_store_items\""));
        assert!(sql.contains("WHERE file_path = $1"));
    }

    #[test]
    fn test_cosine_similarity() {
        assert!((FileVectorStore::cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!((FileVectorStore::cosine(&[1.0, 0.0], &[0.0, 1.0])).abs() < 1e-6);
        assert_eq!(FileVectorStore::cosine(&[], &[]), 0.0);
    }
}
