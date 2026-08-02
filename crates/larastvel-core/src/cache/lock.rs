use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use uuid::Uuid;

use super::CacheError;

/// The lock-persistence contract behind [`Lock`], mirroring Laravel's
/// cache locks (`Cache::lock('key', $seconds)`).
#[async_trait]
pub trait LockStore: Send + Sync + std::fmt::Debug {
    /// Try to take the lock; returns `Ok(true)` when acquired.
    async fn acquire(&self, key: &str, owner: &str, ttl: Duration) -> Result<bool, CacheError>;
    /// Release the lock, but only when `owner` currently holds it.
    async fn release(&self, key: &str, owner: &str) -> Result<bool, CacheError>;
    /// Release the lock regardless of the owner.
    async fn force_release(&self, key: &str) -> Result<bool, CacheError>;
    /// Extend the lock's TTL when `owner` still holds it (Laravel 13.17's
    /// `Lock::refresh()`).
    async fn refresh(&self, key: &str, owner: &str, ttl: Duration) -> Result<bool, CacheError>;
}

/// A named lock over a [`LockStore`]. Use `CacheManager::lock()` to obtain
/// one, then `get()` to acquire it and `release()` when done.
#[derive(Debug, Clone)]
pub struct Lock {
    store: Arc<dyn LockStore>,
    key: String,
    owner: String,
    ttl: Duration,
}

impl Lock {
    pub fn new(store: Arc<dyn LockStore>, key: &str, ttl: Duration) -> Self {
        Self {
            store,
            key: key.to_string(),
            owner: Uuid::new_v4().to_string(),
            ttl,
        }
    }

    /// The unique owner token of this lock instance.
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Try to acquire the lock once.
    pub async fn get(&self) -> Result<bool, CacheError> {
        self.store.acquire(&self.key, &self.owner, self.ttl).await
    }

    /// Release the lock if this instance still owns it.
    pub async fn release(&self) -> Result<bool, CacheError> {
        self.store.release(&self.key, &self.owner).await
    }

    /// Release the lock even if a different owner holds it.
    pub async fn force_release(&self) -> Result<bool, CacheError> {
        self.store.force_release(&self.key).await
    }

    /// Extend the lock's TTL (Laravel's `Lock::refresh()`).
    pub async fn refresh(&self) -> Result<bool, CacheError> {
        self.store.refresh(&self.key, &self.owner, self.ttl).await
    }

    /// Block (polling every 100ms) until the lock is acquired or `timeout`
    /// elapses. Returns whether the lock was acquired.
    pub async fn block(&self, timeout: Duration) -> Result<bool, CacheError> {
        let deadline = Instant::now() + timeout;
        loop {
            if self.get().await? {
                return Ok(true);
            }
            if Instant::now() >= deadline {
                return Ok(false);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

/// In-process [`LockStore`] used by [`super::array::ArrayStore`]. Suitable
/// for tests and single-server processes; locks are not shared across
/// processes.
#[derive(Debug, Clone, Default)]
pub struct ArrayLockStore {
    locks: Arc<Mutex<HashMap<String, (String, Instant)>>>,
}

impl ArrayLockStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LockStore for ArrayLockStore {
    async fn acquire(&self, key: &str, owner: &str, ttl: Duration) -> Result<bool, CacheError> {
        let mut locks = self.locks.lock().unwrap();
        locks.retain(|_, (_, expiry)| *expiry > Instant::now());
        match locks.get(key) {
            Some((existing, _)) if existing != owner => Ok(false),
            _ => {
                locks.insert(key.to_string(), (owner.to_string(), Instant::now() + ttl));
                Ok(true)
            }
        }
    }

    async fn release(&self, key: &str, owner: &str) -> Result<bool, CacheError> {
        let mut locks = self.locks.lock().unwrap();
        match locks.get(key) {
            Some((existing, _)) if existing == owner => {
                locks.remove(key);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn force_release(&self, key: &str) -> Result<bool, CacheError> {
        Ok(self.locks.lock().unwrap().remove(key).is_some())
    }

    async fn refresh(&self, key: &str, owner: &str, ttl: Duration) -> Result<bool, CacheError> {
        let mut locks = self.locks.lock().unwrap();
        match locks.get(key) {
            Some((existing, _)) if existing == owner => {
                locks.insert(key.to_string(), (owner.to_string(), Instant::now() + ttl));
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Arc<dyn LockStore> {
        Arc::new(ArrayLockStore::new())
    }

    fn lock(store: &Arc<dyn LockStore>, ttl: Duration) -> Lock {
        Lock::new(store.clone(), "resource", ttl)
    }

    #[tokio::test]
    async fn acquire_and_release() {
        let s = store();
        let l = lock(&s, Duration::from_secs(60));
        assert!(l.get().await.unwrap());
        assert!(l.release().await.unwrap());
        assert!(!l.release().await.unwrap());
    }

    #[tokio::test]
    async fn second_owner_blocked() {
        let s = store();
        let first = lock(&s, Duration::from_secs(60));
        assert!(first.get().await.unwrap());
        let second = lock(&s, Duration::from_secs(60));
        assert!(!second.get().await.unwrap());
        // Only the owner can release.
        assert!(!second.release().await.unwrap());
        assert!(first.release().await.unwrap());
        assert!(second.get().await.unwrap());
    }

    #[tokio::test]
    async fn force_release() {
        let s = store();
        let first = lock(&s, Duration::from_secs(60));
        assert!(first.get().await.unwrap());
        let second = lock(&s, Duration::from_secs(60));
        assert!(second.force_release().await.unwrap());
        assert!(second.get().await.unwrap());
    }

    #[tokio::test]
    async fn refresh_extends_ttl() {
        let s = store();
        let l = lock(&s, Duration::from_millis(200));
        assert!(l.get().await.unwrap());
        assert!(l.refresh().await.unwrap());
        tokio::time::sleep(Duration::from_millis(150)).await;
        // Still held because refresh pushed the expiry out.
        assert!(!lock(&s, Duration::from_secs(60)).get().await.unwrap());
    }

    #[tokio::test]
    async fn lock_expires() {
        let s = store();
        let l = lock(&s, Duration::from_millis(50));
        assert!(l.get().await.unwrap());
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(lock(&s, Duration::from_secs(60)).get().await.unwrap());
    }

    #[tokio::test]
    async fn block_waits_for_release() {
        let s = store();
        let held = lock(&s, Duration::from_secs(60));
        assert!(held.get().await.unwrap());
        let waiter = lock(&s, Duration::from_secs(60));
        let held_clone = held.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            held_clone.release().await.unwrap();
        });
        assert!(waiter.block(Duration::from_secs(5)).await.unwrap());
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn block_times_out() {
        let s = store();
        let held = lock(&s, Duration::from_secs(60));
        assert!(held.get().await.unwrap());
        let waiter = lock(&s, Duration::from_secs(60));
        assert!(!waiter.block(Duration::from_millis(150)).await.unwrap());
        assert!(held.release().await.unwrap());
    }
}
