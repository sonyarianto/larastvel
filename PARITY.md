# Parity Tracking

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

~100% feature parity with 611+ unit tests.
