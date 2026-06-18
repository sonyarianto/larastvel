# Middleware

Middleware provides a convenient mechanism for filtering HTTP requests entering your application.

## Defining Middleware

Create a middleware function and register it with the `Registrar`:

```rust
registrar.middleware("auth", |r| {
    r.layer(auth_middleware_layer)
});
```

## Global Middleware

Apply middleware to all subsequent routes:

```rust
registrar.with_middleware(vec!["session", "csrf"]);
```

## Route-Specific Middleware

Middleware can be scoped to route groups:

```rust
registrar.group("/admin", |r| {
    r.with_middleware(vec!["auth", "admin"]);
    r.get("/dashboard", admin_dashboard);
});
```

Middleware state is saved/restored when entering/exiting a group.

## Built-in Middleware

Larastvel ships with auto-wired middleware:

- **Session** — encrypted cookie-based sessions
- **CSRF** — cross-site request forgery protection
- **CORS** — configurable via `cors_middleware()`
- **Request Logger** — via `request_logger()`

```rust
use larastvel_core::middleware::{cors_middleware, request_logger};
```

## Custom Middleware

Apply any Tower/Axum layer directly:

```rust
registrar.middleware("throttle", |r| {
    r.layer(rate_limit_middleware(RateLimitConfig::new(60, 1)))
});
```
