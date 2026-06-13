pub mod array;
pub mod database;
pub mod file;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default TTL of 60 minutes (3600 seconds).
pub const DEFAULT_TTL_SECONDS: u64 = 3600;

/// One year in seconds — used for "forever" cached items.
pub const FOREVER_TTL: u64 = 31536000;

/// Prefix helper — joins a cache key prefix with a key.
pub fn prefixed_key(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{}{}", prefix, key)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Store error: {0}")]
    Store(String),
    #[error("Store [{0}] not configured")]
    NotConfigured(String),
}

/// A single cache entry with its expiry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheItem {
    pub value: String,
    pub expires_at: Option<DateTime<Utc>>,
}

impl CacheItem {
    pub fn new(value: String, ttl_seconds: Option<u64>) -> Self {
        let expires_at =
            ttl_seconds.map(|secs| Utc::now() + chrono::Duration::seconds(secs as i64));
        Self { value, expires_at }
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => Utc::now() > expiry,
            None => false,
        }
    }
}

/// Trait for cache store implementations.
#[async_trait]
pub trait CacheStore: Send + Sync + std::fmt::Debug {
    /// Retrieve a value by key. Returns `None` if missing or expired.
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError>;

    /// Store a value with an optional TTL in seconds.
    async fn set(&self, key: &str, value: &str, ttl_seconds: Option<u64>)
        -> Result<(), CacheError>;

    /// Store a value permanently (or for a very long time).
    async fn forever(&self, key: &str, value: &str) -> Result<(), CacheError> {
        self.set(key, value, Some(FOREVER_TTL)).await
    }

    /// Delete a key. Returns `true` if the key existed.
    async fn delete(&self, key: &str) -> Result<bool, CacheError>;

    /// Check if a key exists and is not expired.
    async fn has(&self, key: &str) -> Result<bool, CacheError> {
        Ok(self.get(key).await?.is_some())
    }

    /// Remove all items from the store.
    async fn clear(&self) -> Result<(), CacheError>;

    /// Get and delete a key atomically.
    async fn pull(&self, key: &str) -> Result<Option<String>, CacheError> {
        let value = self.get(key).await?;
        if value.is_some() {
            self.delete(key).await?;
        }
        Ok(value)
    }

    /// Increment a numeric value. Returns the new value.
    async fn increment(&self, key: &str, by: i64) -> Result<i64, CacheError>;

    /// Decrement a numeric value. Returns the new value.
    async fn decrement(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        self.increment(key, -by).await
    }

    /// Retrieve multiple keys at once.
    async fn many(&self, keys: &[&str]) -> Result<HashMap<String, Option<String>>, CacheError> {
        let mut result = HashMap::new();
        for key in keys {
            let value = self.get(key).await?;
            result.insert(key.to_string(), value);
        }
        Ok(result)
    }

    /// Store multiple items at once with the same TTL.
    async fn set_many(
        &self,
        items: &[(&str, &str)],
        ttl_seconds: Option<u64>,
    ) -> Result<(), CacheError> {
        for (key, value) in items {
            self.set(key, value, ttl_seconds).await?;
        }
        Ok(())
    }

    /// Get or store a value using a boxed callback.
    async fn remember(
        &self,
        key: &str,
        ttl_seconds: u64,
        callback: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = String> + Send>> + Send>,
    ) -> Result<String, CacheError> {
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }
        let value = callback().await;
        self.set(key, &value, Some(ttl_seconds)).await?;
        Ok(value)
    }

    /// Get or store forever using a boxed callback.
    async fn remember_forever(
        &self,
        key: &str,
        callback: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = String> + Send>> + Send>,
    ) -> Result<String, CacheError> {
        self.remember(key, FOREVER_TTL, callback).await
    }

    fn name(&self) -> &str;
}

/// Manager for multiple cache stores.
#[derive(Debug, Clone)]
pub struct CacheManager {
    stores: HashMap<String, Arc<dyn CacheStore>>,
    default: String,
}

impl CacheManager {
    pub fn new(default: &str) -> Self {
        Self {
            stores: HashMap::new(),
            default: default.to_string(),
        }
    }

    pub fn register<S: CacheStore + 'static>(&mut self, name: &str, store: S) {
        self.stores.insert(name.to_string(), Arc::new(store));
    }

    pub fn store(&self, name: &str) -> Result<Arc<dyn CacheStore>, CacheError> {
        self.stores
            .get(name)
            .cloned()
            .ok_or_else(|| CacheError::NotConfigured(name.to_string()))
    }

    pub fn default_store(&self) -> Result<Arc<dyn CacheStore>, CacheError> {
        self.store(&self.default)
    }

    pub fn set_default(&mut self, name: &str) {
        self.default = name.to_string();
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn store_names(&self) -> Vec<String> {
        self.stores.keys().cloned().collect()
    }

    /// Convenience: get from the default store.
    pub async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        self.default_store()?.get(key).await
    }

    /// Convenience: set on the default store.
    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> Result<(), CacheError> {
        self.default_store()?.set(key, value, ttl_seconds).await
    }

    /// Convenience: forever on the default store.
    pub async fn forever(&self, key: &str, value: &str) -> Result<(), CacheError> {
        self.default_store()?.forever(key, value).await
    }

    /// Convenience: delete from the default store.
    pub async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        self.default_store()?.delete(key).await
    }

    /// Convenience: has on the default store.
    pub async fn has(&self, key: &str) -> Result<bool, CacheError> {
        self.default_store()?.has(key).await
    }

    /// Convenience: clear the default store.
    pub async fn clear(&self) -> Result<(), CacheError> {
        self.default_store()?.clear().await
    }

    /// Convenience: pull from the default store.
    pub async fn pull(&self, key: &str) -> Result<Option<String>, CacheError> {
        self.default_store()?.pull(key).await
    }

    /// Convenience: increment on the default store.
    pub async fn increment(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        self.default_store()?.increment(key, by).await
    }

    /// Convenience: decrement on the default store.
    pub async fn decrement(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        self.default_store()?.decrement(key, by).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::array::ArrayStore;

    fn setup_manager() -> CacheManager {
        let mut manager = CacheManager::new("array");
        manager.register("array", ArrayStore::new("array"));
        manager
    }

    #[tokio::test]
    async fn test_cache_item_not_expired() {
        let item = CacheItem::new("hello".to_string(), Some(3600));
        assert!(!item.is_expired());
    }

    #[tokio::test]
    async fn test_cache_item_expired() {
        let item = CacheItem {
            value: "hello".to_string(),
            expires_at: Some(Utc::now() - chrono::Duration::seconds(1)),
        };
        assert!(item.is_expired());
    }

    #[tokio::test]
    async fn test_cache_item_no_expiry() {
        let item = CacheItem::new("forever".to_string(), None);
        assert!(!item.is_expired());
    }

    #[tokio::test]
    async fn test_prefixed_key() {
        assert_eq!(prefixed_key("", "key"), "key");
        assert_eq!(prefixed_key("prefix:", "key"), "prefix:key");
    }

    #[tokio::test]
    async fn test_manager_get_set() {
        let manager = setup_manager();
        assert!(manager.get("name").await.unwrap().is_none());

        manager.set("name", "Larastvel", Some(60)).await.unwrap();
        assert_eq!(
            manager.get("name").await.unwrap(),
            Some("Larastvel".to_string())
        );
    }

    #[tokio::test]
    async fn test_manager_has() {
        let manager = setup_manager();
        assert!(!manager.has("key").await.unwrap());
        manager.set("key", "val", Some(60)).await.unwrap();
        assert!(manager.has("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_delete() {
        let manager = setup_manager();
        manager.set("del", "me", Some(60)).await.unwrap();
        assert!(manager.has("del").await.unwrap());
        assert!(manager.delete("del").await.unwrap());
        assert!(!manager.has("del").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_delete_missing() {
        let manager = setup_manager();
        assert!(!manager.delete("nonexistent").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_forever() {
        let manager = setup_manager();
        manager.forever("forever", "always").await.unwrap();
        assert_eq!(
            manager.get("forever").await.unwrap(),
            Some("always".to_string())
        );
    }

    #[tokio::test]
    async fn test_manager_pull() {
        let manager = setup_manager();
        manager.set("pull", "me", Some(60)).await.unwrap();
        assert_eq!(manager.pull("pull").await.unwrap(), Some("me".to_string()));
        assert!(!manager.has("pull").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_clear() {
        let manager = setup_manager();
        manager.set("a", "1", Some(60)).await.unwrap();
        manager.set("b", "2", Some(60)).await.unwrap();
        manager.clear().await.unwrap();
        assert!(!manager.has("a").await.unwrap());
        assert!(!manager.has("b").await.unwrap());
    }

    #[tokio::test]
    async fn test_manager_increment() {
        let manager = setup_manager();
        assert_eq!(manager.increment("counter", 1).await.unwrap(), 1);
        assert_eq!(manager.increment("counter", 5).await.unwrap(), 6);
    }

    #[tokio::test]
    async fn test_manager_decrement() {
        let manager = setup_manager();
        manager.set("count", "10", None).await.unwrap();
        assert_eq!(manager.decrement("count", 3).await.unwrap(), 7);
    }

    #[tokio::test]
    async fn test_manager_store_not_configured() {
        let manager = CacheManager::new("default");
        let result = manager.store("nonexistent");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager_store_names() {
        let mut manager = CacheManager::new("array");
        manager.register("array", ArrayStore::new("array"));
        manager.register("file", ArrayStore::new("file"));
        let mut names = manager.store_names();
        names.sort();
        assert_eq!(names, vec!["array", "file"]);
    }

    #[tokio::test]
    async fn test_manager_set_default() {
        let mut manager = CacheManager::new("first");
        manager.register("first", ArrayStore::new("first"));
        manager.register("second", ArrayStore::new("second"));
        manager.set_default("second");
        assert_eq!(manager.default_name(), "second");
    }

    #[tokio::test]
    async fn test_cache_error_messages() {
        let err = CacheError::Store("disk full".to_string());
        assert_eq!(err.to_string(), "Store error: disk full");

        let err = CacheError::NotConfigured("redis".to_string());
        assert_eq!(err.to_string(), "Store [redis] not configured");
    }

    #[tokio::test]
    async fn test_remember() {
        let manager = setup_manager();
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c = called.clone();

        let result = manager
            .default_store()
            .unwrap()
            .remember(
                "remembered",
                60,
                Box::new(move || {
                    let c = c.clone();
                    Box::pin(async move {
                        c.store(true, std::sync::atomic::Ordering::SeqCst);
                        "computed".to_string()
                    })
                }),
            )
            .await
            .unwrap();

        assert_eq!(result, "computed");
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));

        // Second call should use cache
        let called2 = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let c2 = called2.clone();
        let result2 = manager
            .default_store()
            .unwrap()
            .remember(
                "remembered",
                60,
                Box::new(move || {
                    let c2 = c2.clone();
                    Box::pin(async move {
                        c2.store(true, std::sync::atomic::Ordering::SeqCst);
                        "recomputed".to_string()
                    })
                }),
            )
            .await
            .unwrap();

        assert_eq!(result2, "computed");
        assert!(!called2.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_many() {
        let manager = setup_manager();
        manager.set("k1", "v1", Some(60)).await.unwrap();
        manager.set("k2", "v2", Some(60)).await.unwrap();

        let results = manager
            .default_store()
            .unwrap()
            .many(&["k1", "k2", "k3"])
            .await
            .unwrap();

        assert_eq!(results.get("k1").unwrap(), &Some("v1".to_string()));
        assert_eq!(results.get("k2").unwrap(), &Some("v2".to_string()));
        assert_eq!(results.get("k3").unwrap(), &None);
    }

    #[tokio::test]
    async fn test_set_many() {
        let manager = setup_manager();
        manager
            .default_store()
            .unwrap()
            .set_many(&[("a", "1"), ("b", "2")], Some(60))
            .await
            .unwrap();

        assert_eq!(manager.get("a").await.unwrap(), Some("1".to_string()));
        assert_eq!(manager.get("b").await.unwrap(), Some("2".to_string()));
    }
}
