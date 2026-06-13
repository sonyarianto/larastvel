use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use super::{prefixed_key, CacheError, CacheStore};

/// Database-backed cache store using SeaORM.
///
/// Stores cache items in a database table (defaults to `cache`).
/// This is the equivalent of Laravel's `database` cache driver.
///
/// The table schema:
/// ```sql
/// CREATE TABLE IF NOT EXISTS cache (
///     key TEXT PRIMARY KEY,
///     value TEXT NOT NULL,
///     expiration INTEGER NOT NULL DEFAULT 0
/// );
/// ```
#[derive(Debug, Clone)]
pub struct DatabaseStore {
    name: String,
    db: sea_orm::DatabaseConnection,
    table: String,
    prefix: String,
}

impl DatabaseStore {
    /// Create a new database-backed cache store.
    ///
    /// - `name`: The store name (e.g. "database").
    /// - `db`: A SeaORM database connection.
    /// - `table`: The cache table name (defaults to "cache").
    /// - `prefix`: An optional key prefix.
    pub fn new(name: &str, db: sea_orm::DatabaseConnection, table: &str, prefix: &str) -> Self {
        Self {
            name: name.to_string(),
            db,
            table: table.to_string(),
            prefix: prefix.to_string(),
        }
    }

    /// Ensure the cache table exists.
    pub async fn ensure_table_exists(&self) -> Result<(), CacheError> {
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                expiration INTEGER NOT NULL DEFAULT 0
            )",
            self.table
        );
        self.db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| CacheError::Store(format!("Failed to create cache table: {}", e)))?;
        Ok(())
    }

    fn prefixed(&self, key: &str) -> String {
        prefixed_key(&self.prefix, key)
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

#[async_trait]
impl CacheStore for DatabaseStore {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let prefixed = self.prefixed(key);
        let now = Self::now();

        let sql = format!(
            "SELECT value, expiration FROM {} WHERE key = ?1",
            self.table
        );

        let result = self
            .db
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [prefixed.into()],
            ))
            .await
            .map_err(|e| CacheError::Store(e.to_string()))?;

        match result {
            Some(row) => {
                let value: String = row
                    .try_get_by_index(0)
                    .map_err(|e| CacheError::Deserialization(e.to_string()))?;
                let expiration: i64 = row
                    .try_get_by_index(1)
                    .map_err(|e| CacheError::Deserialization(e.to_string()))?;

                if expiration > 0 && expiration <= now {
                    // Expired — remove it
                    let _ = self.delete(key).await;
                    Ok(None)
                } else {
                    Ok(Some(value))
                }
            }
            None => Ok(None),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> Result<(), CacheError> {
        let prefixed = self.prefixed(key);
        let now = Self::now();
        let expiration = match ttl_seconds {
            Some(secs) => now + secs as i64,
            None => now + super::FOREVER_TTL as i64,
        };

        let sql = format!(
            "INSERT OR REPLACE INTO {} (key, value, expiration) VALUES (?1, ?2, ?3)",
            self.table
        );

        self.db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [prefixed.into(), value.to_string().into(), expiration.into()],
            ))
            .await
            .map_err(|e| CacheError::Store(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let prefixed = self.prefixed(key);
        let sql = format!("DELETE FROM {} WHERE key = ?1", self.table);

        let result = self
            .db
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                &sql,
                [prefixed.into()],
            ))
            .await
            .map_err(|e| CacheError::Store(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let sql = format!("DELETE FROM {}", self.table);

        self.db
            .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
            .await
            .map_err(|e| CacheError::Store(e.to_string()))?;

        Ok(())
    }

    async fn increment(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        let current = self
            .get(key)
            .await?
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let new = current + by;
        self.set(key, &new.to_string(), None).await?;
        Ok(new)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    async fn setup_store() -> DatabaseStore {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");
        let store = DatabaseStore::new("database", db, "cache", "");
        store.ensure_table_exists().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_db_store_get_set() {
        let store = setup_store().await;
        assert!(store.get("key").await.unwrap().is_none());

        store.set("key", "value", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_db_store_overwrite() {
        let store = setup_store().await;
        store.set("key", "old", Some(60)).await.unwrap();
        store.set("key", "new", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("new".to_string()));
    }

    #[tokio::test]
    async fn test_db_store_delete() {
        let store = setup_store().await;
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.delete("key").await.unwrap());
        assert!(!store.delete("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_db_store_has() {
        let store = setup_store().await;
        assert!(!store.has("key").await.unwrap());
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.has("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_db_store_clear() {
        let store = setup_store().await;
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        store.clear().await.unwrap();
        assert!(!store.has("a").await.unwrap());
        assert!(!store.has("b").await.unwrap());
    }

    #[tokio::test]
    async fn test_db_store_pull() {
        let store = setup_store().await;
        store.set("key", "val", Some(60)).await.unwrap();
        assert_eq!(store.pull("key").await.unwrap(), Some("val".to_string()));
        assert!(!store.has("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_db_store_increment() {
        let store = setup_store().await;
        assert_eq!(store.increment("counter", 1).await.unwrap(), 1);
        assert_eq!(store.increment("counter", 5).await.unwrap(), 6);
    }

    #[tokio::test]
    async fn test_db_store_decrement() {
        let store = setup_store().await;
        store.set("count", "10", None).await.unwrap();
        assert_eq!(store.decrement("count", 3).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn test_db_store_forever() {
        let store = setup_store().await;
        store.forever("key", "always").await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("always".to_string()));
    }

    #[tokio::test]
    async fn test_db_store_name() {
        let store = setup_store().await;
        assert_eq!(store.name(), "database");
    }

    #[tokio::test]
    async fn test_db_store_prefix() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        let store = DatabaseStore::new("db", db, "cache", "prefix:");
        store.ensure_table_exists().await.unwrap();

        store.set("key", "val", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("val".to_string()));
    }

    #[tokio::test]
    async fn test_db_store_many() {
        let store = setup_store().await;
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        let results = store.many(&["a", "b", "c"]).await.unwrap();
        assert_eq!(results.get("a").unwrap(), &Some("1".to_string()));
        assert_eq!(results.get("b").unwrap(), &Some("2".to_string()));
        assert_eq!(results.get("c").unwrap(), &None);
    }

    #[tokio::test]
    async fn test_db_store_remember() {
        let store = setup_store().await;
        let result = store
            .remember(
                "computed",
                60,
                Box::new(|| Box::pin(async { "expensive".to_string() })),
            )
            .await
            .unwrap();
        assert_eq!(result, "expensive");
        assert_eq!(
            store.get("computed").await.unwrap(),
            Some("expensive".to_string())
        );
    }

    #[tokio::test]
    async fn test_db_store_custom_table() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect");
        let store = DatabaseStore::new("db", db, "app_cache", "");
        store.ensure_table_exists().await.unwrap();

        store.set("key", "val", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("val".to_string()));
    }

    #[tokio::test]
    async fn test_db_store_expired() {
        let store = setup_store().await;
        store.set("temp", "gone", Some(0)).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(store.get("temp").await.unwrap().is_none());
    }
}
