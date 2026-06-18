# Rate Limiting

Larastvel provides token-bucket rate limiting with configurable limits.

## Configuration

```rust
use larastvel_core::rate_limiter::{
    RateLimiter, RateLimiterRegistry, RateLimitConfig, rate_limit_middleware,
};

let mut registry = RateLimiterRegistry::new();
registry.register("api", RateLimiter::new(60, 1)); // 60 requests per second

// Apply to routes
router.middleware("throttle", |r| {
    r.layer(rate_limit_middleware(RateLimitConfig::new(60, 1)))
});
```

## Global Rate Limiter

```rust
use larastvel_core::rate_limiter::rate_limiter;

let limiter = rate_limiter("api", 60, 1);
if limiter.check("client-ip").await {
    // allowed
} else {
    // rate limited
}
```

## Custom Limits

```rust
let strict = RateLimiter::new(10, 60);  // 10 requests per 60 seconds
let generous = RateLimiter::new(1000, 3600); // 1000 per hour
```
