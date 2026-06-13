use std::path::PathBuf;

use md5::{Digest, Md5};

use async_trait::async_trait;
use tokio::fs;

use super::{prefixed_key, CacheError, CacheItem, CacheStore};

/// File-based cache store.
///
/// Each cache key is serialized as a JSON file on disk. Expired items are
/// cleaned on read. This is the equivalent of Laravel's `file` cache driver.
#[derive(Debug, Clone)]
pub struct FileStore {
    name: String,
    directory: PathBuf,
    prefix: String,
}

impl FileStore {
    /// Create a new file-based cache store.
    ///
    /// - `name`: The store name (e.g. "file").
    /// - `directory`: The root directory for cache files (e.g. "storage/cache/data").
    /// - `prefix`: An optional key prefix.
    pub fn new(name: &str, directory: PathBuf, prefix: &str) -> Self {
        Self {
            name: name.to_string(),
            directory,
            prefix: prefix.to_string(),
        }
    }

    fn path_for(&self, key: &str) -> PathBuf {
        let prefixed = prefixed_key(&self.prefix, key);
        // Hash the key to avoid filesystem issues with special characters
        let hashed = hex::encode(Md5::digest(prefixed.as_bytes()));
        self.directory.join(&hashed)
    }

    async fn read_item(&self, key: &str) -> Result<Option<CacheItem>, CacheError> {
        let path = self.path_for(key);
        if !path.exists() {
            return Ok(None);
        }
        let contents = fs::read_to_string(&path).await?;
        let item: CacheItem = serde_json::from_str(&contents)
            .map_err(|e| CacheError::Deserialization(e.to_string()))?;
        if item.is_expired() {
            let _ = fs::remove_file(&path).await;
            return Ok(None);
        }
        Ok(Some(item))
    }

    async fn write_item(&self, key: &str, item: &CacheItem) -> Result<(), CacheError> {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let contents =
            serde_json::to_string(item).map_err(|e| CacheError::Serialization(e.to_string()))?;
        fs::write(&path, &contents).await?;
        Ok(())
    }
}

#[async_trait]
impl CacheStore for FileStore {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let item = self.read_item(key).await?;
        Ok(item.map(|i| i.value))
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> Result<(), CacheError> {
        let item = CacheItem::new(value.to_string(), ttl_seconds);
        self.write_item(key, &item).await
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let path = self.path_for(key);
        if path.exists() {
            fs::remove_file(&path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn clear(&self) -> Result<(), CacheError> {
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory).await?;
            fs::create_dir_all(&self.directory).await?;
        }
        Ok(())
    }

    async fn increment(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        let current = self
            .read_item(key)
            .await?
            .and_then(|item| item.value.parse::<i64>().ok())
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
    use rand::Rng;

    fn temp_dir() -> PathBuf {
        let mut rng = rand::rngs::OsRng;
        let suffix: u64 = rng.gen();
        std::env::temp_dir().join(format!("larastvel_cache_file_test_{}", suffix))
    }

    async fn cleanup(dir: &std::path::Path) {
        if dir.exists() {
            fs::remove_dir_all(dir).await.unwrap();
        }
    }

    fn create_store() -> (FileStore, PathBuf) {
        let dir = temp_dir();
        let store = FileStore::new("file", dir.clone(), "");
        (store, dir)
    }

    #[tokio::test]
    async fn test_file_store_get_set() {
        let (store, dir) = create_store();
        assert!(store.get("key").await.unwrap().is_none());

        store.set("key", "value", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("value".to_string()));
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_overwrite() {
        let (store, dir) = create_store();
        store.set("key", "old", Some(60)).await.unwrap();
        store.set("key", "new", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("new".to_string()));
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_delete() {
        let (store, dir) = create_store();
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.delete("key").await.unwrap());
        assert!(!store.delete("key").await.unwrap());
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_has() {
        let (store, dir) = create_store();
        assert!(!store.has("key").await.unwrap());
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.has("key").await.unwrap());
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_clear() {
        let (store, dir) = create_store();
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        store.clear().await.unwrap();
        assert!(!store.has("a").await.unwrap());
        assert!(!store.has("b").await.unwrap());
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_pull() {
        let (store, dir) = create_store();
        store.set("key", "val", Some(60)).await.unwrap();
        assert_eq!(store.pull("key").await.unwrap(), Some("val".to_string()));
        assert!(!store.has("key").await.unwrap());
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_increment() {
        let (store, dir) = create_store();
        assert_eq!(store.increment("counter", 1).await.unwrap(), 1);
        assert_eq!(store.increment("counter", 5).await.unwrap(), 6);
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_decrement() {
        let (store, dir) = create_store();
        store.set("count", "10", None).await.unwrap();
        assert_eq!(store.decrement("count", 3).await.unwrap(), 7);
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_forever() {
        let (store, dir) = create_store();
        store.forever("key", "always").await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("always".to_string()));
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_name() {
        let (store, dir) = create_store();
        assert_eq!(store.name(), "file");
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_many() {
        let (store, dir) = create_store();
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        let results = store.many(&["a", "b", "c"]).await.unwrap();
        assert_eq!(results.get("a").unwrap(), &Some("1".to_string()));
        assert_eq!(results.get("b").unwrap(), &Some("2".to_string()));
        assert_eq!(results.get("c").unwrap(), &None);
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_remember() {
        let (store, dir) = create_store();
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
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_clear_only_cache_dir() {
        let (store, dir) = create_store();
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.has("key").await.unwrap());
        store.clear().await.unwrap();
        assert!(!store.has("key").await.unwrap());
        // Directory should still exist after clear
        assert!(dir.exists());
        cleanup(&dir).await;
    }

    #[tokio::test]
    async fn test_file_store_expired_returns_none() {
        let (store, dir) = create_store();
        // TTL of 0 seconds = expired immediately
        store.set("temp", "gone", Some(0)).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        assert!(store.get("temp").await.unwrap().is_none());
        cleanup(&dir).await;
    }
}
