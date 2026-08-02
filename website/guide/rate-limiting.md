# Rate Limiting

Larastvel provides token-bucket rate limiting with configurable limits.

## Configuration

```rust
use larastvel_core::rate_limiter::{
    rate_limiter, RateLimiter, RateLimiterRegistry, RateLimitConfig, rate_limit_middleware,
};

let mut registry = RateLimiterRegistry::new();
let limiter = RateLimiter::new(RateLimitConfig::per_second(60).named("api"));
registry.register(limiter); // keyed by the config's name ("api")

// Apply the middleware (it reads the registry from request extensions)
// router.middleware("throttle", rate_limit_middleware)
```

## Global Rate Limiter

```rust
use larastvel_core::rate_limiter::rate_limiter;
use larastvel_core::rate_limiter::RateLimitConfig;

let limiter = rate_limiter(RateLimitConfig::per_second(60).named("api"));

if limiter.too_many_attempts("client-ip") {
    // rate limited
} else {
    limiter.hit("client-ip"); // record the attempt
    // allowed
}
```

## Custom Limits

```rust
let strict = RateLimiter::new(RateLimitConfig::per_minute(10));   // 10 requests per 60 seconds
let generous = RateLimiter::new(RateLimitConfig::per_hour(1000)); // 1000 per hour
```

## Limiter API

```rust
limiter.hit("client-ip");              // record an attempt, returns the new attempt count
limiter.too_many_attempts("client-ip");// has the limit been reached?
limiter.attempts("client-ip");         // attempts in the current window
limiter.remaining("client-ip");        // attempts left before the limit
limiter.retry_after("client-ip");      // seconds until the window resets
limiter.reset("client-ip");            // clear the window for an identifier
```

`RateLimitConfig` constructors: `per_second(n)`, `per_minute(n)`, `per_hour(n)`, all chainable with `.named("name")` (used as the registry key).
