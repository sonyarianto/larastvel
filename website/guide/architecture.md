# Architecture

## Overview

```
┌──────────────────────────────────────────────────────┐
│                     Application                        │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │  Config  │  │    DB    │  │  Service Container │  │
│  │  (TOML)  │  │ (SeaORM) │  │  (TypeId-based)    │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
│  ┌────────────────────────────────────────────────┐  │
│  │           Router (Axum + Registrar)            │  │
│  │    Routes → Groups → Middleware → Controllers  │  │
│  └────────────────────────────────────────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │ Session  │  │  Cache   │  │  Queue / Events    │  │
│  │ + CSRF   │  │ (stores) │  │  + Notifications   │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

## Crate Layout

The project is a Cargo workspace with 7 crates:

| Crate | Purpose |
|---|---|
| `larastvel-core` | Framework core — router, DB, config, views, middleware |
| `larastvel-cli` | Artisan-like CLI binary |
| `larastvel-macros` | Procedural macros (`Resource`, `controller`, `route`) |
| `larastvel-tinker` | Interactive REPL binary |
| `larastvel-new` | Project scaffolding binary |
| `larastvel-testing` | Test utilities (`TestClient`, `TestResponse`, `RefreshDatabase`) |
| `larastvel-app` (root) | Application entrypoint |

## Request Lifecycle

1. **HTTP request** arrives at Axum server
2. **SessionLayer** decrypts cookies, loads session data
3. **CsrfLayer** validates CSRF tokens (excluded for API/health routes)
4. **User middleware** runs (auth, rate limiting, etc.)
5. **Router** matches route → calls handler
6. **Handler** returns response (possibly via Tera view)
7. **Response** sent back through middleware layers in reverse

## Application Builder

```rust
App::new()
    .config(Config::load())
    .database(DatabaseManager::new(&config))
    .registrar(|r| {
        web(&r);
        api(&r);
    })
    .with_layer(MyCustomLayer)
    .run()
    .await
```

Session and CSRF layers are auto-wired when `app.key` is present in config.
