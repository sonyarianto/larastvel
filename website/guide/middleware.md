# Middleware

Middleware provides a convenient mechanism for filtering HTTP requests entering your application.

## Defining Middleware

Create a middleware function and register it with the `Registrar`:

```rust
registrar.middleware("auth", |r| {
    r.layer(axum::middleware::from_fn(auth_middleware))
});
```

## Middleware Presets

Larastvel ships with ready-to-use middleware presets that can be registered by name:

```rust
use larastvel_core::middleware::presets;

registrar.middleware("auth", presets::auth());
registrar.middleware("cors", presets::cors());
registrar.middleware("log", presets::logger());
registrar.middleware("verified", presets::verified());
registrar.middleware("guest", presets::guest());
registrar.middleware("throttle:api", presets::throttle(
    larastvel_core::rate_limiter::RateLimitConfig::per_minute(60),
));
```

| Preset | Description |
|--------|-------------|
| `auth()` | JWT bearer-token authentication (returns 401 on failure) |
| `cors()` | Permissive `Access-Control-Allow-Origin: *` headers |
| `logger()` | Request/response logging via `tracing::info!` |
| `verified()` | Rejects unverified email users (returns 403) |
| `guest()` | Rejects authenticated users (returns 403) — for login/register pages |
| `throttle(config)` | In-memory rate limiting, returns 429 with `Retry-After` |

The `guest` preset must be placed after `auth` in the middleware stack (it checks for `AuthenticatedUser` in request extensions).

## Per-Route Middleware with `#[middleware]`

Use the `#[middleware]` attribute inside `#[route]` impl blocks to attach middleware to individual handlers:

```rust
use larastvel_core::{route, get, middleware};

#[route]
impl MyController {
    #[get("/public")]
    async fn public_endpoint() -> &'static str {
        "public"
    }

    #[get("/dashboard")]
    #[middleware("auth", "verified")]
    async fn dashboard() -> &'static str {
        "dashboard"
    }
}
```

Routes without `#[middleware]` receive no middleware. The named middleware must be registered with `registrar.middleware(...)` before `register_routes` is called.

## Global Middleware

Apply middleware to all subsequent routes:

```rust
registrar.with_middleware(vec!["session", "csrf"]);
```

## Route Groups

Middleware can be scoped to route groups:

```rust
registrar.group("/admin", |r| {
    r.with_middleware(vec!["auth", "admin"]);
    r.get("/dashboard", admin_dashboard);
});
```

Middleware state is saved and restored when entering/exiting a group.

## Auto-Wired Middleware

Larastvel automatically wires the following middleware when an app key is configured:

- **Session** — encrypted cookie-based sessions
- **CSRF** — cross-site request forgery protection (excludes `/api/*` and `/health`)

## Custom Middleware

Apply any Tower/Axum layer directly:

```rust
registrar.middleware("custom", |r| {
    r.layer(my_custom_layer)
});
```
