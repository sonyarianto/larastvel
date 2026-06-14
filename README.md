# Larastvel

A Rust web framework inspired by Laravel, built on Axum, Tokio, and SeaORM.

## Status

Active development (~88% feature parity). Core architecture is solid with most framework features implemented.

## Features

- **Routing** — Expressively define routes with groups, inspired by Laravel's Router
- **Database** — SeaORM-powered connection manager (SQLite, PostgreSQL, MySQL) with migrations
- **Config** — TOML-based config with dot-notation access + `.env` support
- **Templating** — Tera template engine (Blade-like syntax)
- **CLI** — Artisan-equivalent tool for `serve`, `make:*`, `key:generate`, `route:list`, `migrate:*`, `db:seed`
- **Service Container** — TypeId-based IoC container with `bind`/`singleton`/`make`
- **Logging** — Structured tracing with env-filter support
- **Middleware** — CORS, request logging, auth, rate limiting, session (Tower-based)
- **Vite Integration** — Asset bundling with manifest-based tag generation
- **Macros** — `#[derive(Resource)]`, `#[controller]`, `#[route]` proc macros
- **Tinker** — Interactive REPL (early stage)
- **Scaffolding** — `larastvel-new` to generate new projects
- **Authentication** — JWT-based `Auth` service + `AuthenticatedUser` extractor + auth middleware
- **Encryption / Hashing** — AES-256-GCM encryption + bcrypt hashing
- **Form Validation** — 20 built-in rules, `ValidatedJson`/`ValidatedQuery` extractors, 422 response
- **Pagination** — Laravel-compatible JSON with `Paginator<T>`, `PaginationParams` extractor
- **Session** — Encrypted cookie-based sessions with flash, CSRF, `SessionLayer` middleware
- **Events / Listeners** — `EventService` with `dispatch()`, `listen()`, `fake()` / `assertDispatched()`
- **File Storage** — `Filesystem` trait, `LocalDisk` driver, `StorageManager`
- **Notifications** — `NotificationSender`, `Notification` trait, `Notifiable` trait, 5 channels (Mail, Database, Broadcast, SMS, Webhook), multi-channel `via()`, per-channel result inspection, `send_all()`
- **SMS** — `SmsSender` trait, `LogSmsSender`, `VonageSmsSender` (REST API), `SmsMessage` builder
- **Mail** — `Mailable` builder, `SmtpMailer` (STARTTLS), `LogMailer`, `MailManager`
- **Queue / Jobs** — `SyncQueue`, `InMemoryQueue`, `DatabaseQueue`, `QueueWorker`, `dispatch()`
- **Rate Limiting** — Token bucket with `RateLimiter`, `RateLimiterRegistry`, Axum middleware
- **Localization** — JSON translation files, `__()` / `trans_choice()`, pluralization, locale switching
- **Task Scheduling** — Cron expression parser, `Schedule` builder, `ScheduleManager`
- **Cache** — `CacheManager` with multiple stores (array, file, database), TTL support, remember, batch operations
- **Password Reset** — `PasswordResetBroker` with token generation, database-backed token storage, throttle/expiry, reset link email via `Mailable`, `reset()` with password update callback
- **Email Verification** — `EmailVerificationBroker` with JWT-signed tokens / `VerifiedUser` Axum extractor / `require_verified_email` middleware / `send_verification_email()` / `mark_verified()` callback / `email_verified_at` column in users table
- **Testing** — 500+ unit tests + 100 example tests across all modules

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
examples/             Controllers & examples (mail, SMS, notification, password-reset, auth service provider, multi-channel, unified dashboard)
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

## Parity Tracking

A fresh Laravel 13 installation lives at [`../laravel-skeleton/`](../laravel-skeleton/) for side-by-side comparison. Use it to verify directory structure, config defaults, routing conventions, and feature behavior.

| Laravel Feature | Larastvel Equivalent | Status |
|---|---|---|
| `routes/web.php` | `src/routes/web.rs` | ✅ |
| `routes/api.php` | `src/routes/api.rs` | ✅ |
| `routes/console.php` | `crates/larastvel-cli/src/main.rs` | ⚠️ Partial |
| `config/*.php` (10 files) | `config.toml` (single file) | ⚠️ Partial |
| `.env` | `.env` | ✅ |
| `bootstrap/app.php` | `foundation::Application` / `bootstrap::App` fluent builder | ✅ |
| `app/Providers/*` | `EventServiceProvider` / `RouteServiceProvider` / `ServiceProvider` trait / `DeferrableProvider` trait / deferred registration & boot | ✅ |
| `artisan` CLI (25+ commands) | `larastvel-cli` (serve, route:list, key:generate, migrate*, db:seed, storage:link, notifications:table, queue:work, config:cache/clear, route:cache/clear, schedule:list/run, version, new, make:*) | ✅ |
| `make:*` (model, controller, migration, seeder, policy, test, job, event, notification, rule, command) | `larastvel make:*` — 12 generators | ✅ |
| `app/Http/Controllers/` | `#[controller]` / `#[derive(Resource)]` macros | ✅ |
| `app/Models/User.php` | `src/models/user.rs` | ✅ |
| Eloquent ORM | `DbModel` trait + SeaORM / `SerializesToArray` (toArray/toJson/hidden/appends) / `ApiResource` trait / `JsonResource` / `ResourceCollection` | ✅ |
| Model Factories (Faker) | `ModelFactory` trait / `factory_create()` / `factory_create_count()` / `Faker` (name/email/sentence/etc.) | ✅ |
| Blade templating | Tera + Blade directive compiler (@auth/@csrf/@error/@guest/@method) | ✅ |
| Migrations (`database/migrations/`) | `src/database/migrations/` + Migrator | ✅ |
| `php artisan migrate` | `larastvel migrate` | ✅ |
| Seeders | `Seeder` trait + `DatabaseManager::seed::<S>()` + `make:seeder` | ✅ |
| Session | `SessionHandle` extractor / `SessionLayer` middleware / flash / CSRF / encrypted cookie store | ✅ |
| Authentication / Auth | JWT `Auth` service + `AuthenticatedUser` extractor + `auth_middleware` | ✅ |
| Password Reset | `PasswordResetBroker` / token generation / throttle / expiry / `send_reset_link()` / `reset()` with callback / `password_reset_tokens` table | ✅ |
| Email Verification | `EmailVerificationBroker` / JWT-signed tokens / `VerifiedUser` extractor / `require_verified_email` middleware / `send_verification_email()` / `mark_verified()` / `email_verified_at` column | ✅ |
| Authorization / Gates | `Gate` / `Policy` trait / `require_ability` middleware / `authorize()` / `check_ability()` / before/after hooks / ability inspection | ✅ |
| Queue / Jobs | `SyncQueue` / `InMemoryQueue` / `DatabaseQueue` / `QueueWorker` / `QueueManager` / `dispatch()` / `ShouldQueue` trait | ✅ |
| Notifications / Mail | `NotificationSender` / `Notification` trait / `Notifiable` trait / 5 channels (Mail, Database, Broadcast, SMS, Webhook) / `SmsSender` trait / `LogSmsSender` / `VonageSmsSender` / `Mailer` trait / `Mailable` builder / `SmtpMailer` / `LogMailer` / `MailManager` | ✅ |
| File Storage (Flysystem) | `Filesystem` trait / `LocalDisk` driver / `StorageManager` / put/get/delete/copy/move/list/dirs | ✅ |
| Events / Listeners | `EventService` / `dispatch()` / `listen()` / `fake()` / `Listener` trait | ✅ |
| Form Validation | `Validator` / `validate()` / `ValidationErrors` + 20 built-in rules | ✅ |
| Pagination | `Paginator<T>` / `PaginationParams` extractor / `to_json()` / `IntoResponse` | ✅ |
| Rate Limiting | `RateLimiter` / `RateLimiterRegistry` / `rate_limit_middleware` / token bucket | ✅ |
| Encryption / Hashing | `hash::make()` / `hash::check()` / `Encrypter` | ✅ |
| Broadcasting | `BroadcastManager` / `PusherBroadcaster` / `LogBroadcaster` / `AblyBroadcaster` / `NativeBroadcaster` (self-hosted WebSocket) / `SubscriberRegistry` / `ws_handler` / `Channel` (public/private/presence) / `BroadcastEvent` trait | ✅ |
| Cache (config/cache.php) | `CacheManager` / `ArrayStore` / `FileStore` / `DatabaseStore` / `CacheStore` trait / `CacheItem` with TTL / `remember()` / `many()` / increment/decrement | ✅ |
| Localization | `Translator` / `__()` / `trans_choice()` / pluralization / `set_locale()` / JSON files | ✅ |
| Testing (PHPUnit) | `TestClient` / `TestResponse` / `RefreshDatabase` / PHPUnit-like assertions | ✅ |
| Task Scheduling (Cron) | `Schedule` / `ScheduleManager` / cron parser / `ScheduledEvent` / `schedule:run` CLI | ✅ |

## License

MIT
