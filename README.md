# Larastvel

[![CI](https://github.com/sonyarianto/larastvel/actions/workflows/ci.yml/badge.svg)](https://github.com/sonyarianto/larastvel/actions)
![Rust](https://img.shields.io/badge/rust-stable-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

A Rust web framework inspired by Laravel, built on Axum, Tokio, and SeaORM.
~100% feature parity with 611+ unit tests.

---

## Quick Start

```bash
# Clone and run
cargo run
# → http://localhost:8080
```

### Routes

Define routes in `src/routes/`:

```rust
// src/routes/web.rs
pub fn web(router: &Registrar) {
    router.get("/", || async {
        axum::response::Html("<h1>Welcome</h1>")
    });
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

The `config/` directory holds per-section TOML files. Missing sections use
built-in defaults.

```toml
# config/app.toml
name = "Larastvel"
url = "http://localhost:8080"
env = "local"
debug = true
key = ""    # generate with `larastvel key:generate`
```

```toml
# config/database.toml
driver = "sqlite"   # sqlite, postgres, mysql
host = "127.0.0.1"
port = 3306
database = "larastvel.db"
username = "root"
password = ""
```

See [Configuration Reference](#configuration-reference) for all options.

### Generate a new project

```bash
cargo run -p larastvel-new -- my-app
cd my-app
cargo run
```

### CLI

```bash
cargo run -p larastvel-cli -- serve          # Start dev server
cargo run -p larastvel-cli -- key:generate   # Generate encryption key
cargo run -p larastvel-cli -- make model User
cargo run -p larastvel-cli -- route:list
```

---

## Features

| Area | Capabilities |
|---|---|
| **Routing** | Groups, prefixes, middleware stack, `#[controller]` / `#[derive(Resource)]` macros, WebSocket routes |
| **Database** | SQLite/Postgres/MySQL via SeaORM, migrations, seeders, model factories (Faker) |
| **Auth** | JWT tokens, `AuthenticatedUser` extractor, auth middleware, password reset, email verification |
| **Authorization** | Gates, policies, before/after hooks, `authorize()` / `require_ability` |
| **Session** | Encrypted cookie store, flash data, CSRF protection, `SessionLayer` middleware (auto-wired) |
| **Caching** | `CacheManager` with array, file, database stores, TTL, `remember()`, batch ops |
| **Queue** | Sync, in-memory, database queues, worker, `dispatch()`, `ShouldQueue` |
| **Notifications** | Mail, Database, Broadcast, SMS, Webhook channels, multi-channel `via()` |
| **Mail** | SMTP (STARTTLS) and log mailers, `Mailable` builder, `MailManager` |
| **SMS** | Log and Vonage senders, `SmsMessage` builder |
| **Broadcasting** | Pusher, Ably, Log, Native (self-hosted WebSocket) broadcast drivers |
| **Validation** | 20 built-in rules, `ValidatedJson`/`ValidatedQuery` extractors |
| **Templating** | Tera engine + Blade directives (`@auth`, `@csrf`, `@error`, `@guest`, `@method`) |
| **Localization** | JSON translation files, `__()`, `trans_choice()`, pluralization |
| **Task Scheduling** | Cron expression parser, `Schedule` builder, `ScheduleManager` |
| **Rate Limiting** | Token bucket, `RateLimiterRegistry`, Axum middleware |
| **File Storage** | `Filesystem` trait, `LocalDisk` driver, `StorageManager` |
| **Events** | `EventService`, `dispatch()`, `listen()`, `fake()` / `assertDispatched()` |
| **Encryption** | AES-256-GCM (`Encrypter`), bcrypt hashing (`hash::make` / `hash::check`) |
| **CLI** | 12 `make:*` generators, `serve`, `migrate`, `route:list`, `config:cache`, `schedule:run`, `queue:work`, and more |
| **Testing** | `TestClient`, `TestResponse`, `RefreshDatabase`, 611+ tests |
| **Vite** | Manifest-based asset tag generation |
| **Scaffolding** | `larastvel-new` generates a complete project with routes, models, migrations, Vite |

---

## Configuration Reference

| File | Key | Default | Description |
|---|---|---|---|
| `app.toml` | `name` | `"Larastvel"` | Application name |
| | `url` | `"http://localhost:8080"` | Base URL |
| | `env` | `"local"` | Environment (`local`, `production`, `testing`) |
| | `debug` | `true` | Enable debug output |
| | `key` | none | 32-byte base64 encryption key (generate via `key:generate`) |
| `database.toml` | `driver` | `"sqlite"` | `sqlite`, `postgres`, `mysql` |
| | `host` | `"127.0.0.1"` | Database host |
| | `port` | `3306` | Database port |
| | `database` | `"larastvel"` | Database name / SQLite filename |
| | `username` | `"root"` | Database user |
| | `password` | `""` | Database password |
| `logging.toml` | `level` | `"debug"` | Log level |
| | `format` | `"text"` | Output format (`text`, `json`) |
| `view.toml` | `engine` | `"tera"` | Template engine |
| | `paths` | `["resources/views"]` | Template search paths |
| `broadcasting.toml` | `default` | `"log"` | Default driver |
| | `app_id` / `key` / `secret` | `""` | Pusher/Ably credentials |
| | `cluster` | `"mt1"` | Pusher cluster |
| | `encrypted` | `true` | TLS for Pusher |
| `cache.toml` | `default` | `"array"` | Cache driver |
| | `prefix` | `""` | Key prefix |
| | `table` | `"cache"` | DB table (database driver) |
| | `file_path` | `"storage/framework/cache/data"` | File path (file driver) |
| `password_reset.toml` | `table` | `"password_reset_tokens"` | DB table |
| | `expire_seconds` | `3600` | Token lifetime |
| | `throttle_seconds` | `60` | Min seconds between resets |

A single `config.toml` at the project root still works (legacy format), but
`config/` takes precedence.

---

## Examples

Ready-to-run examples in `examples/`:

| Example | What it demonstrates |
|---|---|
| `auth_service_provider` | Auth, password reset, email verification working together |
| `multi_channel` | Broadcasting on multiple channels |
| `unified_dashboard` | WebSocket dashboard with broadcast log, auth, rate limiting |
| `websocket_broadcast` | Self-hosted WebSocket via NativeBroadcaster |
| `mail`, `sms`, `notification` | Mail/SMS/Notification sending |
| `password_reset` | Password reset flow |

Run any example: `cargo run --example <name>`

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                   Application                         │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │  Config   │  │   DB     │  │  Service Container │  │
│  │  (TOML)   │  │  (SeaORM)│  │  (TypeId-based)    │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
│  ┌──────────────────────────────────────────────────┐ │
│  │              Router (Axum + Registrar)            │ │
│  │  Routes → Groups → Middleware → Controllers      │ │
│  └──────────────────────────────────────────────────┘ │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │ Session  │  │  Cache   │  │  Queue / Events    │  │
│  │ + CSRF   │  │  (stores)│  │  + Notifications   │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

```
crates/
  larastvel-core/     Framework core (router, DB, config, view, middleware, etc.)
  larastvel-cli/      Artisan-like CLI binary
  larastvel-macros/   Procedural macros (Resource, controller, route)
  larastvel-tinker/   Interactive REPL binary
  larastvel-new/      Project scaffolding binary
  larastvel-testing/  Test utilities (TestClient, TestResponse, RefreshDatabase)
src/                  Application entrypoint
config/               Per-section TOML config files
resources/            Views, CSS, JS
examples/             Self-contained example apps
```

---

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
| Asset bundling | Vite | Vite (manifest-based) |

---

## Parity Tracking

A fresh Laravel 13 installation lives at `../laravel-skeleton/` for
side-by-side comparison.

| Laravel Feature | Larastvel Equivalent | Status |
|---|---|---|
| `routes/web.php` | `src/routes/web.rs` | ✅ |
| `routes/api.php` | `src/routes/api.rs` | ✅ |
| `routes/console.php` | `Command` trait / `ConsoleKernel` / `routes/console.rs` | ✅ |
| `config/*.php` (10 files) | `config/*.toml` (per-section files) | ✅ |
| `.env` | `.env` | ✅ |
| `bootstrap/app.php` | `Application` / `App` fluent builder | ✅ |
| `app/Providers/*` | `ServiceProvider` trait, `EventServiceProvider`, `RouteServiceProvider`, deferred providers | ✅ |
| `artisan` CLI (25+ commands) | `larastvel-cli` — serve, route:list, key:generate, migrate*, db:seed, storage:link, notifications:table, queue:work, config:cache/clear, route:cache/clear, schedule:list/run, version, new, make:* | ✅ |
| `make:*` (12 generators) | `larastvel make:*` — model, controller, migration, seeder, policy, test, job, event, notification, rule, command | ✅ |
| `app/Http/Controllers/` | `#[controller]` / `#[derive(Resource)]` macros | ✅ |
| `app/Models/User.php` | `src/models/user.rs` | ✅ |
| Eloquent ORM | `DbModel` trait + SeaORM + `SerializesToArray` / `ApiResource` / `JsonResource` / `ResourceCollection` | ✅ |
| Model Factories (Faker) | `ModelFactory` trait, `factory_create()`, `Faker` helpers | ✅ |
| Blade templating | Tera + Blade directives (`@auth`/`@csrf`/`@error`/`@guest`/`@method`) | ✅ |
| Migrations | `src/database/migrations/` + Migrator | ✅ |
| `php artisan migrate` | `larastvel migrate` | ✅ |
| Seeders | `Seeder` trait + `DatabaseManager::seed::<S>()` + `make:seeder` | ✅ |
| Session | `SessionHandle` extractor / `SessionLayer` middleware / flash / CSRF / encrypted cookies | ✅ |
| Authentication | JWT `Auth` service + `AuthenticatedUser` extractor + `auth_middleware` | ✅ |
| Password Reset | `PasswordResetBroker` / tokens / throttle / expiry / reset email / callback | ✅ |
| Email Verification | `EmailVerificationBroker` / JWT-signed tokens / `VerifiedUser` extractor / middleware | ✅ |
| Authorization / Gates | `Gate` / `Policy` / `require_ability` middleware / before/after hooks | ✅ |
| Queue / Jobs | `SyncQueue` / `InMemoryQueue` / `DatabaseQueue` / `QueueWorker` / `dispatch()` / `ShouldQueue` | ✅ |
| Notifications / Mail | 5 channels (Mail, Database, Broadcast, SMS, Webhook), `Mailable` builder, `SmtpMailer` / `LogMailer` | ✅ |
| File Storage | `Filesystem` trait / `LocalDisk` driver / `StorageManager` | ✅ |
| Events / Listeners | `EventService` / `dispatch()` / `listen()` / `fake()` / `Listener` trait | ✅ |
| Form Validation | 20 rules, `ValidatedJson`/`ValidatedQuery` extractors | ✅ |
| Pagination | `Paginator<T>` / `PaginationParams` / `to_json()` / `IntoResponse` | ✅ |
| Rate Limiting | `RateLimiter` / `RateLimiterRegistry` / middleware / token bucket | ✅ |
| Encryption / Hashing | AES-256-GCM `Encrypter` / bcrypt `hash::make()` / `hash::check()` | ✅ |
| Broadcasting | Pusher / Ably / Log / Native (WebSocket) / `SubscriberRegistry` / `ws_handler` | ✅ |
| Cache | `CacheManager` / Array / File / Database stores / `remember()` / batch ops | ✅ |
| Localization | `Translator` / `__()` / `trans_choice()` / pluralization / JSON files | ✅ |
| Testing | `TestClient` / `TestResponse` / `RefreshDatabase` | ✅ |
| Task Scheduling | `Schedule` / `ScheduleManager` / cron parser / `schedule:run` CLI | ✅ |

---

## Development

```bash
# Run all tests
cargo test --workspace

# Check formatting
cargo fmt --check

# Lint
cargo clippy --workspace

# Run a specific example
cargo run --example unified_dashboard
```

---

## License

MIT
