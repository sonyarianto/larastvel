# Larastvel

A Rust web framework inspired by Laravel, built on Axum, Tokio, and SeaORM.

## Status

Early prototype (~20-30% feature parity). Core architecture is in place, but most features are stubs or skeletons.

## Features

- **Routing** — Expressively define routes with groups, inspired by Laravel's Router
- **Database** — SeaORM-powered connection manager (SQLite, PostgreSQL, MySQL)
- **Config** — TOML-based config with dot-notation access + `.env` support
- **Templating** — Tera template engine (Blade-like syntax)
- **CLI** — Artisan-equivalent tool for `serve`, `make:*`, `key:generate`, `route:list`
- **Service Container** — TypeId-based IoC container with `bind`/`singleton`/`make`
- **Logging** — Structured tracing with env-filter support
- **Middleware** — CORS and request logging (Tower-based)
- **Vite Integration** — Asset bundling with manifest-based tag generation
- **Macros** — `#[derive(Resource)]`, `#[controller]`, `#[route]` proc macros
- **Tinker** — Interactive REPL (early stage)
- **Scaffolding** — `larastvel-new` to generate new projects

## Quick Start

```bash
cargo run
```

Then visit `http://localhost:8080`.

### Routes

Defined in `src/routes/`:

```rust
// src/routes/web.rs
pub fn web(router: &Registrar) {
    router.get("/", || async { axum::response::Html("<h1>Welcome</h1>") });
}

// src/routes/api.rs
pub fn api(router: &Registrar) {
    router.group("/api", |r| {
        r.get("/health", || async {
            axum::response::Json(serde_json::json!({"status": "ok"}))
        });
    });
}
```

### Configuration

Edit `config.toml` or set environment variables in `.env`.

```toml
[app]
name = "Larastvel"
url = "http://localhost:8080"
env = "local"
debug = true

[database]
driver = "sqlite"
database = "larastvel.db"
```

### CLI

```bash
cargo run -p larastvel-cli -- serve
cargo run -p larastvel-cli -- key:generate
cargo run -p larastvel-cli -- make:model User
cargo run -p larastvel-cli -- route:list
```

### Scaffold a new project

```bash
cargo run -p larastvel-new -- my-app
```

## Workspace Structure

```
crates/
  larastvel-core/     Framework core (router, DB, config, view, middleware, etc.)
  larastvel-cli/      Artisan-like CLI binary
  larastvel-macros/   Procedural macros (Resource, controller, route)
  larastvel-tinker/   Interactive REPL binary
  larastvel-new/      Project scaffolding binary
src/                  Application entrypoint
resources/            Views, CSS, JS (Laravel resources/ equivalent)
config.toml           Application configuration
```

## Tech Stack

| Concern | Laravel | Larastvel |
|---|---|---|
| HTTP | Symfony/Illuminate | Axum 0.8 |
| Runtime | PHP-FPM | Tokio |
| ORM | Eloquent | SeaORM 1.x |
| Templating | Blade | Tera |
| CLI | Artisan | Clap |
| Config | PHP arrays / `.env` | TOML / `.env` |
| Logging | Monolog | Tracing |
| Migrations | Phinx | sea-orm-migration |
| Asset bundling | Vite | Vite (via rust-embed) |

## License

MIT
