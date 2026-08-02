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
| `make:*` (19 generators) | `larastvel make:*` — model, controller, migration, seeder, policy, test, job, event, listener, notification, rule, command, factory, mail, scope, observer, resource, provider, broadcast-event | ✅ |
| `app/Http/Controllers/` | `#[controller]` / `#[derive(Resource)]` macros | ✅ |
| `app/Models/User.php` | `src/models/user.rs` | ✅ |
| Eloquent ORM | `DbModel` trait + SeaORM + `SerializesToArray` / `ApiResource` / `JsonResource` / `ResourceCollection` | ✅ |
| Model Factories (Faker) | `ModelFactory` trait, `factory_create()`, `Faker` helpers | ✅ |
| Blade templating | Tera + Blade directives (`@auth`/`@csrf`/`@error`/`@guest`/`@method`) | ✅ |
| Migrations | `src/database/migrations/` + Migrator | ✅ |
| `php artisan migrate` | `larastvel migrate` | ✅ |
| Seeders | `Seeder` trait + `DatabaseManager::seed::<S>()` + `make seeder` | ✅ |
| Session | `SessionHandle` extractor / `SessionLayer` middleware / flash / CSRF / encrypted cookies | ✅ |
| Authentication | JWT `Auth` service + `AuthenticatedUser` extractor + `auth_middleware` | ✅ |
| Password Reset | `PasswordResetBroker` / tokens / throttle / expiry / reset email / callback | ✅ |
| Email Verification | `EmailVerificationBroker` / JWT-signed tokens / `VerifiedUser` extractor / middleware | ✅ |
| Passkey authentication (WebAuthn) | `PasskeyService` / `PasskeyStore` / `MemoryPasskeyStore` — registration & assertion options, challenge verification (origin, flags, rpIdHash, counter), ES256 via ring | ✅ |
| Authorization / Gates | `Gate` / `Policy` / `require_ability` middleware / before/after hooks | ✅ |
| Queue / Jobs | `SyncQueue` / `InMemoryQueue` / `DatabaseQueue` / `QueueWorker` / `dispatch()` / `ShouldQueue` / retries with backoff, timeout, `fail_on_timeout`, `#[job(delay)]` delayed delivery (`ShouldQueue::delay_seconds()`) | ✅ |
| Failed job handling | `FailedJobStore` / `Queue::fail(exception)` / `queue:failed` / `queue:retry` / `queue:forget` / `queue:flush` | ✅ |
| DB transactions | `DatabaseManager::transaction()` / `begin_transaction()` | ✅ |
| Notifications / Mail | 5 channels (Mail, Database, Broadcast, SMS, Webhook), `Mailable` builder, `SmtpMailer` / `LogMailer` | ✅ |
| File Storage | `Filesystem` trait / `LocalDisk` driver / `StorageManager` | ✅ |
| Events / Listeners | `EventService` / `dispatch()` / `listen()` / `fake()` / `Listener` trait | ✅ |
| Form Validation | 26 rules (incl. `base64`, `active_url`, DB-backed `unique`/`unique_except`/`exists`), `ValidatedJson`/`ValidatedQuery` extractors | ✅ |
| Validation DB rules | `unique` / `unique:except` / `exists` (SQL-backed, async validation via `validate_async()` / `#[validate]`) | ✅ |
| `active_url` rule + DNS lookup faking | `active_url()` rule (real DNS resolution) + `fake_dns_lookups(bool)` — offline tests skip only the network call (Laravel 13.22 `Validator::fakeDnsLookups()`) | ✅ |
| `email:dns` validation option (Laravel 13.22) | `email_dns()` rule — validates email format then requires the domain to resolve in DNS (skipped while `fake_dns_lookups(true)`) | ✅ |
| Route model binding | `ModelPath<E>` extractor — implicit `{user}` → model by primary key, 404 on missing | ✅ |
| Route-key binding (`#[RouteKey]`, Laravel 13.21) | `RouteKey` trait + `#[table("…", route_key = "column")]` — binds `{post}` by slug-style column instead of the primary key, 404 on unknown key | ✅ |
| Route conflict detection | `route:conflicts` — detects overlapping route definitions (duplicates + static shadowing `{param}`/`*`) | ✅ |
| Route metadata | `Registrar::route_with_metadata()` / `RouteDefinition::metadata` — survives route caching | ✅ |
| Signed URLs | `signed_route()` / `has_valid_signature()` — HMAC-SHA256 (RFC 2104), canonical query sorting, TTL expiry, constant-time comparison | ✅ |
| Global helpers | `redirect()` / `back()` / `abort()` / `abort_if()` / `abort_unless()` | ✅ |
| Job batches | `JobBatch` / `batch()` / `dispatch_batch()` / `JobBatchStore` — progress, failed count, cancel (worker skips cancelled jobs), `finished_at` | ✅ |
| Concurrency | `concurrent()` — run boxed async tasks in parallel, results in input order | ✅ |
| Process | `ProcessBuilder` / `run()` / `foreground()` — output capture, env, cwd, timeout kill | ✅ |
| Lazy collections | `LazyCollection` — lazy chainable iterator (filter/map/take/skip/chain/reduce) | ✅ |
| Blade components & slots | `Component` trait / `@component` / `<x-card>` / `<x-slot:name>` / `@slot` — named slots + default `$slot` rendered through Tera | ✅ |
| `artisan about` / `optimize` / `config:show` | `larastvel about` / `optimize` / `optimize:clear` / `config:show {section}` | ✅ |
| Pagination | `Paginator<T>` / `PaginationParams` / `to_json()` / `IntoResponse` | ✅ |
| Rate Limiting | `RateLimiter` / `RateLimiterRegistry` / middleware / token bucket | ✅ |
| Encryption / Hashing | AES-256-GCM `Encrypter` / bcrypt `hash::make()` / `hash::check()` | ✅ |
| Logging | `Log::init()` console logging + `monthly` file driver (`MonthlyWriter` — `laravel-YYYY-MM.log` per calendar month, `max_files` retention) | ✅ |
| Broadcasting | Pusher / Ably / Log / Native (WebSocket) / `SubscriberRegistry` / `ws_handler` / Reverb DB scaling driver (`ReverbDatabaseBroadcaster` + `reverb_scaling` table) | ✅ |
| Cache | `CacheManager` / Array / File / Database / Redis stores / `remember()` / batch ops / `touch()` TTL extension / atomic locks — `CacheManager::lock()` / `store_lock()` / `with_lock()` / `get_locked()`, `Lock::get/release/refresh/block` (Array + Redis: `SET NX PX`, Lua compare-and-release) | ✅ |
| Maintenance mode | `MaintenanceMode` (`down` file under `storage/framework/`), `larastvel down/up`, timing-safe `--secret` bypass (`hash_equals`-style comparison, Laravel 13.23 PR #60896) | ✅ |
| CookieJar (Laravel 13.24 `queued()` fix) | `Cookie` / `CookieJar` — queued cookies keyed by `(name, path)` so same-name cookies on different paths don't clobber; `queue()` / `queued(name, path?)` / `unqueue()` / `forget()` / `to_set_cookie_headers()` | ✅ |
| Localization | `Translator` / `__()` / `trans_choice()` / pluralization / JSON files | ✅ |
| Testing | `TestClient` / `TestResponse` / `RefreshDatabase` | ✅ |
| Task Scheduling | `Schedule` / `ScheduleManager` / cron parser / `schedule:run` CLI | ✅ |
| Scheduled-event timezone & next run | `ScheduledEvent::timezone("Asia/Jakarta")` (IANA, applied to `is_due`) + `next_run()` — computed in the event timezone, returned in local time (`schedule:list` "Next Run") | ✅ |
| Queue routing | `QueueManager::route()` / `routed_queue()` / central job→queue rules | ✅ |
| Pagination default | 25 per page (Laravel 13 default) | ✅ |
| JSON:API resources (relationship inclusion, sparse fieldsets, links) | `JsonApiResource` trait + `JsonApiItem` / `JsonApiCollection` — `?include=` compound documents, `?fields[type]=` sparse fieldsets, `when_included()`, `application/vnd.api+json` | ✅ |
| AI SDK foundation — text generation, streaming, structured output, embeddings | `Ai` facade + `AiProvider` trait — `generate()`, `chat()`, `chat_stream()`, `structured()`, `embed()` with 30-day caching, OpenAI-compatible HTTP provider, `FakeAi` for tests | ✅ |
| AI SDK agents / media (agents, images, audio, TTS/STT, vector stores, reranking) | `Ai::agent()` — persona prompt, `AgentTool` tool calling loop (JSON-schema tools, error recovery, turn limit), `AgentTask` / `AgentResult` / `AgentTaskStatus`; failover via `chat_with_fallback()` / `generate_with_fallback()`; `Media` value type; image create/edit/variation; TTS / STT; moderation; reranking (`Ai::rerank()`); vector stores (`FileVectorStore`, `PostgresVectorStore` + pgvector) | ✅ |
| First-party image processing (`Image` facade, Laravel 13.20) | `Image` facade + `ImageInstance` — `from_bytes/base64/path/url/storage`, `resize/scale/cover/crop/contain/rotate/grayscale/blur/sharpen/flip`, outputs (`to_png/jpg/webp/gif/bmp`), `save`/`store`/`store_publicly`, `dominant_color`, `dimensions`; test fake `Image::fake()` + `assert_resized`/`assert_covered`/`assert_cropped`/`assert_stored` | ✅ |
| Container attribute `#[BindWhen]` (Laravel 13.22) | `#[bind_when(alias, condition_key)]` attribute + generated `<Trait>ConditionalBindings` registrar; container `bind_if()` / `bind_if_config()` / `bind_default()` — conditions evaluated at resolve time against live config | ✅ |

## Laravel 13 gaps (not yet implemented)

| Laravel 13 Feature | Larastvel Equivalent | Status |
|---|---|---|
| Laravel AI SDK (agents, embeddings, audio, images, vector stores) | `Ai` facade + agents with tool calling, failover, `Media`, image generation/editing/variation, TTS / STT, moderation, reranking, and vector stores (`FileVectorStore`, `PostgresVectorStore`) | ✅ |
| Semantic / vector search (`whereVectorSimilarTo()`, pgvector) | `VectorSimilarityQuery` — cosine / L2 / inner product on `Select<E>` | ✅ |
| `PreventRequestForgery` (origin-aware CSRF) | `Sec-Fetch-Site` origin verification, `allow_same_site()` / `use_origin_only()` | ✅ |
| Job attributes (`#[Tries]`, `#[Backoff]`, `#[Timeout]`, `#[FailOnTimeout]`) | `#[job(tries, backoff, timeout, fail_on_timeout)]` with worker retry, delay, timeout enforcement | ✅ |

## Tracked gaps (deferred)

| Laravel 13 Feature | Status | Notes |
|---|---|---|
| _(none currently)_ | — | Previously deferred: first-party image processing and `#[BindWhen]` were implemented in this work cycle (see table above). |

~100% feature parity with 1300+ unit tests (checked against Laravel 13.23.0).
