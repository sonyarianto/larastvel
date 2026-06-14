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

## Usage

```rust
use larastvel_core::cache::CacheManager;

let cache = CacheManager::new(&config);

// Basic
cache.put("key", "value", Duration::from_secs(3600)).await?;
let val: Option<String> = cache.get("key").await?;

// Remember (cache-aside pattern)
let users = cache
    .remember("users", Duration::from_secs(300), || async {
        User::all().await
    })
    .await?;

// Increment / Decrement
cache.increment("counter", 1).await?;
cache.decrement("counter", 1).await?;

// Batch
cache.put_many(vec![("a", "1"), ("b", "2")], Duration::from_secs(60)).await?;
let vals = cache.get_many(&["a", "b"]).await?;
cache.delete_many(&["a", "b"]).await?;

// Clear
cache.flush().await?;
```

## Tagged Cache (Array store)

```rust
cache.tags(&["users", "admins"]).put("user:1", user, ttl).await?;
```

Useful for invalidating groups of cache entries at once.

## Prefix

All keys are automatically prefixed with the configured `prefix` to avoid collisions with other applications sharing the same store.
