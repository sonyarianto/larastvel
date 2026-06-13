use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::{CacheError, CacheItem, CacheStore, FOREVER_TTL};

/// In-memory cache store using a `HashMap`. Useful for testing and single-server
/// use where persistence is not required.
///
/// This is the equivalent of Laravel's `array` cache driver.
#[derive(Debug, Clone)]
pub struct ArrayStore {
    name: String,
    data: Arc<Mutex<HashMap<String, CacheItem>>>,
}

impl ArrayStore {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Remove all expired entries.
    pub fn gc(&self) {
        let mut data = self.data.lock().unwrap();
        data.retain(|_, item| !item.is_expired());
    }

    /// Return the number of items in the store (including expired).
    pub fn len(&self) -> usize {
        self.data.lock().unwrap().len()
    }

    /// Return true if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.data.lock().unwrap().is_empty()
    }
}

#[async_trait]
impl CacheStore for ArrayStore {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let data = self.data.lock().unwrap();
        match data.get(key) {
            Some(item) if !item.is_expired() => Ok(Some(item.value.clone())),
            _ => Ok(None),
        }
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> Result<(), CacheError> {
        let mut data = self.data.lock().unwrap();
        data.insert(
            key.to_string(),
            CacheItem::new(value.to_string(), ttl_seconds),
        );
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let mut data = self.data.lock().unwrap();
        Ok(data.remove(key).is_some())
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut data = self.data.lock().unwrap();
        data.clear();
        Ok(())
    }

    async fn increment(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        let mut data = self.data.lock().unwrap();
        let current = data
            .get(key)
            .filter(|item| !item.is_expired())
            .and_then(|item| item.value.parse::<i64>().ok())
            .unwrap_or(0);
        let new = current + by;
        data.insert(
            key.to_string(),
            CacheItem::new(new.to_string(), Some(FOREVER_TTL)),
        );
        Ok(new)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_array_store_get_set() {
        let store = ArrayStore::new("array");
        assert!(store.get("key").await.unwrap().is_none());

        store.set("key", "value", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_array_store_overwrite() {
        let store = ArrayStore::new("array");
        store.set("key", "old", Some(60)).await.unwrap();
        store.set("key", "new", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("new".to_string()));
    }

    #[tokio::test]
    async fn test_array_store_delete() {
        let store = ArrayStore::new("array");
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.delete("key").await.unwrap());
        assert!(!store.delete("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_array_store_has() {
        let store = ArrayStore::new("array");
        assert!(!store.has("key").await.unwrap());
        store.set("key", "val", Some(60)).await.unwrap();
        assert!(store.has("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_array_store_clear() {
        let store = ArrayStore::new("array");
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        store.clear().await.unwrap();
        assert!(!store.has("a").await.unwrap());
        assert!(!store.has("b").await.unwrap());
    }

    #[tokio::test]
    async fn test_array_store_pull() {
        let store = ArrayStore::new("array");
        store.set("key", "val", Some(60)).await.unwrap();
        assert_eq!(store.pull("key").await.unwrap(), Some("val".to_string()));
        assert!(!store.has("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_array_store_increment() {
        let store = ArrayStore::new("array");
        assert_eq!(store.increment("counter", 1).await.unwrap(), 1);
        assert_eq!(store.increment("counter", 5).await.unwrap(), 6);
        assert_eq!(store.increment("counter", -2).await.unwrap(), 4);
    }

    #[tokio::test]
    async fn test_array_store_decrement() {
        let store = ArrayStore::new("array");
        store.set("count", "10", None).await.unwrap();
        assert_eq!(store.decrement("count", 3).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn test_array_store_forever() {
        let store = ArrayStore::new("array");
        store.forever("key", "always").await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("always".to_string()));
    }

    #[tokio::test]
    async fn test_array_store_many() {
        let store = ArrayStore::new("array");
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();

        let results = store.many(&["a", "b", "c"]).await.unwrap();
        assert_eq!(results.get("a").unwrap(), &Some("1".to_string()));
        assert_eq!(results.get("b").unwrap(), &Some("2".to_string()));
        assert_eq!(results.get("c").unwrap(), &None);
    }

    #[tokio::test]
    async fn test_array_store_set_many() {
        let store = ArrayStore::new("array");
        store
            .set_many(&[("x", "10"), ("y", "20")], Some(60))
            .await
            .unwrap();
        assert_eq!(store.get("x").await.unwrap(), Some("10".to_string()));
        assert_eq!(store.get("y").await.unwrap(), Some("20".to_string()));
    }

    #[tokio::test]
    async fn test_array_store_remember() {
        let store = ArrayStore::new("array");
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
    async fn test_array_store_name() {
        let store = ArrayStore::new("my-cache");
        assert_eq!(store.name(), "my-cache");
    }

    #[tokio::test]
    async fn test_array_store_gc() {
        let store = ArrayStore::new("array");
        store.set("fresh", "ok", Some(60)).await.unwrap();
        store.set("stale", "gone", Some(0)).await.unwrap();
        // Wait a tiny bit for the 0-second TTL to expire
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store.gc();
        assert!(store.has("fresh").await.unwrap());
        assert!(!store.has("stale").await.unwrap());
    }

    #[tokio::test]
    async fn test_array_store_len() {
        let store = ArrayStore::new("array");
        assert_eq!(store.len(), 0);
        store.set("a", "1", Some(60)).await.unwrap();
        assert_eq!(store.len(), 1);
    }
}
