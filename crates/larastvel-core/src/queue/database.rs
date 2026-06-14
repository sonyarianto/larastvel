use std::sync::Arc;

use async_trait::async_trait;

use super::{JobBox, JobError, Queue};

pub type JobResolver = Arc<dyn Fn(&str, &str) -> Option<JobBox> + Send + Sync>;

pub struct DatabaseQueue {
    name: String,
    table_name: String,
    db: sea_orm::DatabaseConnection,
    resolver: JobResolver,
}

impl std::fmt::Debug for DatabaseQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseQueue")
            .field("name", &self.name)
            .field("table_name", &self.table_name)
            .field("db", &self.db)
            .field("resolver", &"<closure>")
            .finish()
    }
}

impl Clone for DatabaseQueue {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            table_name: self.table_name.clone(),
            db: self.db.clone(),
            resolver: self.resolver.clone(),
        }
    }
}

impl DatabaseQueue {
    pub fn new(name: &str, db: sea_orm::DatabaseConnection, resolver: JobResolver) -> Self {
        Self {
            name: name.to_string(),
            table_name: "jobs".to_string(),
            db,
            resolver,
        }
    }

    pub fn with_table(mut self, table: &str) -> Self {
        self.table_name = table.to_string();
        self
    }

    pub async fn ensure_table_exists(&self) -> Result<(), JobError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                queue TEXT NOT NULL DEFAULT 'default',
                payload TEXT NOT NULL,
                class TEXT NOT NULL DEFAULT '',
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                reserved_at INTEGER,
                available_at INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            )",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        self.db
            .execute(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                sql,
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to create jobs table: {}", e)))?;
        Ok(())
    }
}

#[async_trait]
impl Queue for DatabaseQueue {
    async fn push(&self, job: JobBox) -> Result<(), JobError> {
        let payload = serde_json::to_string(&serde_json::json!({
            "name": job.name(),
        }))
        .map_err(|e| JobError::Queue(format!("Serialization error: {}", e)))?;

        let class = job.name().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let sql = format!(
            "INSERT INTO {} (queue, payload, class, attempts, available_at, created_at)
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        self.db
            .execute(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [
                    self.name.clone().into(),
                    payload.into(),
                    class.into(),
                    now.into(),
                ],
            ))
            .await
            .map_err(|e| JobError::Queue(format!("Failed to push job: {}", e)))?;
        Ok(())
    }

    async fn pop(&self) -> Option<JobBox> {
        let sql = format!(
            "SELECT id, payload, class FROM {}
             WHERE queue = ?1 AND (reserved_at IS NULL OR reserved_at < ?2)
             AND attempts < max_attempts
             AND available_at <= ?2
             ORDER BY id ASC LIMIT 1",
            self.table_name
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        use sea_orm::{ConnectionTrait, QueryResult};
        let result: Vec<QueryResult> = self
            .db
            .query_all(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [self.name.clone().into(), now.into(), now.into()],
            ))
            .await
            .ok()?;

        let row = result.into_iter().next()?;

        let class: String = row.try_get_by_index::<String>(2).ok()?;
        let payload: String = row.try_get_by_index::<String>(1).ok()?;

        let resolver = self.resolver.clone();
        let job = resolver(&class, &payload)?;

        let id: Option<i64> = row.try_get_by_index::<i64>(0).ok();
        if let Some(job_id) = id {
            let update_sql = format!(
                "UPDATE {} SET reserved_at = ?1, attempts = attempts + 1 WHERE id = ?2",
                self.table_name
            );
            let _ = self
                .db
                .execute(sea_orm::Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Sqlite,
                    &update_sql,
                    [now.into(), job_id.into()],
                ))
                .await;
        }

        Some(job)
    }

    async fn count(&self) -> usize {
        let sql = format!(
            "SELECT COUNT(*) as cnt FROM {} WHERE queue = ?1 AND attempts < max_attempts",
            self.table_name
        );
        use sea_orm::ConnectionTrait;
        let result = self
            .db
            .query_one(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Sqlite,
                &sql,
                [self.name.clone().into()],
            ))
            .await;

        match result {
            Ok(Some(row)) => {
                let cnt: Option<i64> = row.try_get_by_index(0).ok();
                cnt.unwrap_or(0) as usize
            }
            _ => 0,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}
