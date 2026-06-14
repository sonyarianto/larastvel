# Parity Tracking

Larastvel aims for ~100% feature parity with Laravel. Below is a side-by-side comparison.

| Laravel Feature | Larastvel Equivalent | Status |
|---|---|---|
| `routes/web.php` | `src/routes/web.rs` | ✅ |
| `routes/api.php` | `src/routes/api.rs` | ✅ |
| `routes/console.php` | `Command` / `ConsoleKernel` / `routes/console.rs` | ✅ |
| `config/*.php` (10 files) | `config/*.toml` (per-section files) | ✅ |
| `.env` | `.env` | ✅ |
| `bootstrap/app.php` | `Application` / `App` builder | ✅ |
| `app/Providers/*` | `ServiceProvider`, `EventServiceProvider`, `RouteServiceProvider` | ✅ |
| Artisan CLI (25+ commands) | `larastvel-cli` — serve, route:list, key:generate, migrate, db:seed, make:*, queue:work, etc. | ✅ |
| `make:*` generators | `larastvel make:*` — model, controller, migration, seeder, policy, test, job, event, notification, rule, mail, command | ✅ |
| `app/Http/Controllers/` | `#[controller]` / `#[derive(Resource)]` macros | ✅ |
| Eloquent ORM | `DbModel` + SeaORM + `ApiResource` / `JsonResource` / `ResourceCollection` | ✅ |
| Model Factories (Faker) | `ModelFactory`, `factory_create()`, Faker helpers | ✅ |
| Blade templating | Tera + Blade directives (`@auth`/`@csrf`/`@error`/`@guest`/`@method`) | ✅ |
| Migrations | `src/database/migrations/` + Migrator | ✅ |
| `php artisan migrate` | `larastvel migrate` | ✅ |
| Seeders | `Seeder` trait + `DatabaseManager::seed` | ✅ |
| Session | `SessionHandle` / `SessionLayer` / flash / CSRF / encrypted cookies | ✅ |
| Authentication | JWT `Auth` + `AuthenticatedUser` extractor + `auth_middleware` | ✅ |
| Password Reset | `PasswordResetBroker` / tokens / throttle / expiry | ✅ |
| Email Verification | `EmailVerificationBroker` / JWT tokens / `VerifiedUser` extractor | ✅ |
| Authorization / Gates | `Gate` / `Policy` / `require_ability` middleware | ✅ |
| Queue / Jobs | Sync, InMemory, Database queues / worker / `dispatch()` / `ShouldQueue` | ✅ |
| Notifications / Mail | 5 channels (Mail, Database, Broadcast, SMS, Webhook), `Mailable`, SMTP | ✅ |
| File Storage | `Filesystem` / `LocalDisk` / `StorageManager` | ✅ |
| Events / Listeners | `EventService` / `dispatch()` / `listen()` / `fake()` | ✅ |
| Form Validation | 20 rules, `ValidatedJson`/`ValidatedQuery` extractors | ✅ |
| Pagination | `Paginator<T>` / `PaginationParams` / `IntoResponse` | ✅ |
| Rate Limiting | `RateLimiter` / `RateLimiterRegistry` / middleware | ✅ |
| Encryption / Hashing | AES-256-GCM `Encrypter` / bcrypt `hash::make()` / `hash::check()` | ✅ |
| Broadcasting | Pusher / Ably / Log / Native (WebSocket) | ✅ |
| Cache | `CacheManager` / Array / File / Database stores / `remember()` | ✅ |
| Localization | `Translator` / `__()` / `trans_choice()` / pluralization / JSON files | ✅ |
| Testing | `TestClient` / `TestResponse` / `RefreshDatabase` | ✅ |
| Task Scheduling | `Schedule` / `ScheduleManager` / cron parser | ✅ |

**All features are fully implemented.**
