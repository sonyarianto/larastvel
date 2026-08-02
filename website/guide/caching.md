# Caching

Larastvel provides a `CacheManager` with multiple store backends.

## Configuration

```toml
# config/cache.toml
default = "array"
prefix = ""
table = "cache"
file_path = "storage/framework/cache/data"
```

## Stores

| Driver | Description |
|---|---|
| `array` | In-memory store (default, non-persistent) |
| `file` | File-based store |
| `database` | Database-backed store |
| `redis` | Redis store (native TTL via `EX`/`EXPIRE`) |

### Redis

Construct a `RedisStore` and register it on the `CacheManager` — no store is
auto-wired, so the URL stays explicit:

```rust
use std::sync::Arc;
use larastvel_core::cache::{CacheManager, RedisStore};

let mut manager = CacheManager::new("redis");
let redis = RedisStore::new("redis", "redis://127.0.0.1:6379", "larastvel:")?;
manager.register("redis", redis);
let cache = manager.store("redis")?;

let val: Option<String> = cache.get("user:123").await?;
```

Keys are stored under the store's prefix, so `clear()` only removes keys it
owns (a safe `SCAN` + `DEL` sweep, never `FLUSHDB`).

## Usage

```rust
use larastvel_core::cache::CacheManager;

let cache = CacheManager::new("array");

// Basic
cache.set("key", "value", Some(3600)).await?;
let val: Option<String> = cache.get("key").await?;

// Remember (cache-aside pattern) — available on the store
let store = cache.default_store()?;
let users = store
    .remember("users", 300, Box::new(|| {
        Box::pin(async { load_users().await })
    }))
    .await?;

// Increment / Decrement
cache.increment("counter", 1).await?;
cache.decrement("counter", 1).await?;

// Batch — many / set_many on the store
let store = cache.default_store()?;
store.set_many(&[("a", "1"), ("b", "2")], Some(60)).await?;
let vals = store.many(&["a", "b"]).await?;

// Delete keys individually
store.delete("a").await?;
store.delete("b").await?;

// Touch — extend the TTL of an existing key (seconds, not Duration)
cache.set("key", "value", Some(60)).await?;
cache.touch("key", 3600).await?;

// Clear
cache.clear().await?;
```

## Prefix

All keys are automatically prefixed with the configured `prefix` to avoid collisions with other applications sharing the same store.

## Atomic locks

`CacheManager` provides Laravel-style atomic locks backed by a store's
`LockStore` (array and redis). Redis locks use `SET NX PX` and a Lua
compare-and-release, so cross-process mutual exclusion is safe:

```rust
use std::time::Duration;

let cache = CacheManager::new("array");

// Acquire/release manually
let lock = cache.lock("deploy", Duration::from_secs(60))?;
if lock.get().await? {
    // only one process reaches this
    lock.release().await?;
}

// Block up to 5s for the lock, run the callback, release
cache
    .with_lock("payout", Duration::from_secs(30), Duration::from_secs(5), || async {
        process_payouts().await
    })
    .await?;

// Run the callback only when the lock is free, otherwise return the
// cached value (the callback stores its own result)
cache
    .get_locked("expensive", Duration::from_secs(30), || async {
        let value = expensive_computation().await;
        cache.set("expensive", &value, Some(30)).await?;
        Ok(value)
    })
    .await?;
```

`Lock::block(timeout)` polls every 100 ms; `Lock::refresh()` extends the TTL
(Laravel 13.17 parity), and `Lock::force_release()` breaks a stale lock.
