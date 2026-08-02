use async_trait::async_trait;

use super::{prefixed_key, CacheError, CacheStore};

/// Redis-backed cache store (Laravel's `redis` cache driver).
///
/// Values are stored as plain strings. TTLs use Redis' native `EX` so the
/// server performs expiration; `touch` uses `EXPIRE` without re-fetching.
#[derive(Debug, Clone)]
pub struct RedisStore {
    name: String,
    client: redis::Client,
    prefix: String,
}

impl RedisStore {
    /// Create a new Redis cache store.
    ///
    /// - `name`: The store name (e.g. "redis").
    /// - `url`: A Redis connection URL (e.g. `redis://127.0.0.1:6379`).
    /// - `prefix`: An optional key prefix.
    pub fn new(name: &str, url: &str, prefix: &str) -> Result<Self, CacheError> {
        let client = redis::Client::open(url)
            .map_err(|e| CacheError::Store(format!("Invalid Redis URL: {}", e)))?;
        Ok(Self {
            name: name.to_string(),
            client,
            prefix: prefix.to_string(),
        })
    }

    fn key(&self, key: &str) -> String {
        prefixed_key(&self.prefix, key)
    }

    async fn connection(&self) -> Result<redis::aio::MultiplexedConnection, CacheError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::Store(format!("Redis connection error: {}", e)))
    }
}

#[async_trait]
impl CacheStore for RedisStore {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let mut conn = self.connection().await?;
        let value: Option<String> = redis::cmd("GET")
            .arg(self.key(key))
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Store(format!("Redis GET failed: {}", e)))?;
        Ok(value)
    }

    async fn set(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<u64>,
    ) -> Result<(), CacheError> {
        let mut conn = self.connection().await?;
        let mut cmd = redis::cmd("SET");
        cmd.arg(self.key(key)).arg(value);
        if let Some(ttl) = ttl_seconds {
            cmd.arg("EX").arg(ttl);
        }
        cmd.query_async::<()>(&mut conn)
            .await
            .map_err(|e| CacheError::Store(format!("Redis SET failed: {}", e)))?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, CacheError> {
        let mut conn = self.connection().await?;
        let removed: i64 = redis::cmd("DEL")
            .arg(self.key(key))
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Store(format!("Redis DEL failed: {}", e)))?;
        Ok(removed > 0)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut conn = self.connection().await?;
        // Only remove keys under this store's prefix (SCAN + DEL, chunked).
        let pattern = format!("{}*", self.prefix);
        let mut cursor = 0u64;
        loop {
            let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await
                .map_err(|e| CacheError::Store(format!("Redis SCAN failed: {}", e)))?;
            if !keys.is_empty() {
                let mut del = redis::cmd("DEL");
                for k in &keys {
                    del.arg(k);
                }
                let _: i64 = del
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| CacheError::Store(format!("Redis DEL failed: {}", e)))?;
            }
            if next == 0 {
                break;
            }
            cursor = next;
        }
        Ok(())
    }

    async fn increment(&self, key: &str, by: i64) -> Result<i64, CacheError> {
        let mut conn = self.connection().await?;
        let value: i64 = redis::cmd("INCRBY")
            .arg(self.key(key))
            .arg(by)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Store(format!("Redis INCRBY failed: {}", e)))?;
        Ok(value)
    }

    async fn touch(&self, key: &str, ttl_seconds: u64) -> Result<bool, CacheError> {
        let mut conn = self.connection().await?;
        let touched: i64 = redis::cmd("EXPIRE")
            .arg(self.key(key))
            .arg(ttl_seconds)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Store(format!("Redis EXPIRE failed: {}", e)))?;
        Ok(touched > 0)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    static REDIS_URL: Lazy<String> = Lazy::new(|| {
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
    });

    fn redis_url() -> &'static str {
        &REDIS_URL
    }

    /// Connect if a Redis server is reachable; otherwise skip the test body.
    /// The connect is wrapped in a short timeout so a missing server fails
    /// fast instead of blocking the test suite.
    async fn maybe_store() -> Option<RedisStore> {
        let store = match RedisStore::new("redis", redis_url(), "larastvel_test:") {
            Ok(s) => s,
            Err(_) => return None,
        };
        let connected =
            tokio::time::timeout(std::time::Duration::from_millis(300), store.connection()).await;
        match connected {
            Ok(Ok(_)) => Some(store),
            _ => {
                eprintln!("skipping Redis tests: no server at {}", redis_url());
                None
            }
        }
    }

    #[tokio::test]
    async fn test_redis_store_get_set_delete() {
        let Some(store) = maybe_store().await else {
            return;
        };
        assert!(store.get("key").await.unwrap().is_none());
        store.set("key", "value", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("value".to_string()));
        assert!(store.delete("key").await.unwrap());
        assert!(!store.delete("key").await.unwrap());
    }

    #[tokio::test]
    async fn test_redis_store_overwrite() {
        let Some(store) = maybe_store().await else {
            return;
        };
        store.set("key", "old", Some(60)).await.unwrap();
        store.set("key", "new", Some(60)).await.unwrap();
        assert_eq!(store.get("key").await.unwrap(), Some("new".to_string()));
        let _ = store.delete("key").await;
    }

    #[tokio::test]
    async fn test_redis_store_ttl_expiry() {
        let Some(store) = maybe_store().await else {
            return;
        };
        store.set("temp", "gone", Some(1)).await.unwrap();
        assert!(store.has("temp").await.unwrap());
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        assert!(!store.has("temp").await.unwrap());
    }

    #[tokio::test]
    async fn test_redis_store_forever_has_no_ttl() {
        let Some(store) = maybe_store().await else {
            return;
        };
        store.forever("perm", "always").await.unwrap();
        assert_eq!(store.get("perm").await.unwrap(), Some("always".to_string()));
        let _ = store.delete("perm").await;
    }

    #[tokio::test]
    async fn test_redis_store_clear_only_prefixed() {
        let Some(store) = maybe_store().await else {
            return;
        };
        // An unrelated key (different prefix) must survive clear().
        let other = RedisStore::new("redis", redis_url(), "other:").unwrap();
        other.set("keep", "safe", Some(60)).await.unwrap();
        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        store.clear().await.unwrap();
        assert!(!store.has("a").await.unwrap());
        assert!(!store.has("b").await.unwrap());
        assert!(other.has("keep").await.unwrap());
        let _ = other.delete("keep").await;
    }

    #[tokio::test]
    async fn test_redis_store_increment_decrement() {
        let Some(store) = maybe_store().await else {
            return;
        };
        assert_eq!(store.increment("counter", 1).await.unwrap(), 1);
        assert_eq!(store.increment("counter", 5).await.unwrap(), 6);
        assert_eq!(store.decrement("counter", 2).await.unwrap(), 4);
        let _ = store.delete("counter").await;
    }

    #[tokio::test]
    async fn test_redis_store_pull_and_many() {
        let Some(store) = maybe_store().await else {
            return;
        };
        store.set("p", "val", Some(60)).await.unwrap();
        assert_eq!(store.pull("p").await.unwrap(), Some("val".to_string()));
        assert!(!store.has("p").await.unwrap());

        store.set("a", "1", Some(60)).await.unwrap();
        store.set("b", "2", Some(60)).await.unwrap();
        let results = store.many(&["a", "b", "c"]).await.unwrap();
        assert_eq!(results.get("a").unwrap(), &Some("1".to_string()));
        assert_eq!(results.get("c").unwrap(), &None);
        let _ = store.delete("a").await;
        let _ = store.delete("b").await;
    }

    #[tokio::test]
    async fn test_redis_store_remember() {
        let Some(store) = maybe_store().await else {
            return;
        };
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
        let _ = store.delete("computed").await;
    }

    #[tokio::test]
    async fn test_redis_store_touch() {
        let Some(store) = maybe_store().await else {
            return;
        };
        assert!(!store.touch("missing", 60).await.unwrap());
        store.set("t", "v", Some(60)).await.unwrap();
        assert!(store.touch("t", 120).await.unwrap());
        assert_eq!(store.get("t").await.unwrap(), Some("v".to_string()));
        let _ = store.delete("t").await;
    }

    #[tokio::test]
    async fn test_redis_store_name_and_bad_url() {
        let Some(store) = maybe_store().await else {
            return;
        };
        assert_eq!(store.name(), "redis");
        assert!(RedisStore::new("redis", "not-a-url", "").is_err());
    }
}
