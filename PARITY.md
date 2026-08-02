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
| Authorization / Gates | `Gate` / `Policy` / `require_ability` middleware / before/after hooks | ✅ |
| Queue / Jobs | `SyncQueue` / `InMemoryQueue` / `DatabaseQueue` / `QueueWorker` / `dispatch()` / `ShouldQueue` / retries with backoff, timeout, `fail_on_timeout` | ✅ |
| Failed job handling | `FailedJobStore` / `Queue::fail(exception)` / `queue:failed` / `queue:retry` / `queue:forget` / `queue:flush` | ✅ |
| DB transactions | `DatabaseManager::transaction()` / `begin_transaction()` | ✅ |
| Notifications / Mail | 5 channels (Mail, Database, Broadcast, SMS, Webhook), `Mailable` builder, `SmtpMailer` / `LogMailer` | ✅ |
| File Storage | `Filesystem` trait / `LocalDisk` driver / `StorageManager` | ✅ |
| Events / Listeners | `EventService` / `dispatch()` / `listen()` / `fake()` / `Listener` trait | ✅ |
| Form Validation | 24 rules (incl. DB-backed `unique`/`unique_except`/`exists`), `ValidatedJson`/`ValidatedQuery` extractors | ✅ |
| Validation DB rules | `unique` / `unique:except` / `exists` (SQL-backed, async validation via `validate_async()` / `#[validate]`) | ✅ |
| Route model binding | `ModelPath<E>` extractor — implicit `{user}` → model by primary key, 404 on missing | ✅ |
| Route conflict detection | `route:conflicts` — detects overlapping route definitions (duplicates + static shadowing `{param}`/`*`) | ✅ |
| Route metadata | `Registrar::route_with_metadata()` / `RouteDefinition::metadata` — survives route caching | ✅ |
| Signed URLs | `signed_route()` / `has_valid_signature()` — HMAC-SHA256 (RFC 2104), canonical query sorting, TTL expiry, constant-time comparison | ✅ |
| Global helpers | `redirect()` / `back()` / `abort()` / `abort_if()` / `abort_unless()` | ✅ |
| Job batches | `JobBatch` / `batch()` / `dispatch_batch()` / `JobBatchStore` — progress, failed count, cancel (worker skips cancelled jobs), `finished_at` | ✅ |
| Concurrency | `concurrent()` — run boxed async tasks in parallel, results in input order | ✅ |
| Process | `ProcessBuilder` / `run()` / `foreground()` — output capture, env, cwd, timeout kill | ✅ |
| Lazy collections | `LazyCollection` — lazy chainable iterator (filter/map/take/skip/chain/reduce) | ✅ |
| `artisan about` / `optimize` / `config:show` | `larastvel about` / `optimize` / `optimize:clear` / `config:show {section}` | ✅ |
| Pagination | `Paginator<T>` / `PaginationParams` / `to_json()` / `IntoResponse` | ✅ |
| Rate Limiting | `RateLimiter` / `RateLimiterRegistry` / middleware / token bucket | ✅ |
| Encryption / Hashing | AES-256-GCM `Encrypter` / bcrypt `hash::make()` / `hash::check()` | ✅ |
| Broadcasting | Pusher / Ably / Log / Native (WebSocket) / `SubscriberRegistry` / `ws_handler` | ✅ |
| Cache | `CacheManager` / Array / File / Database stores / `remember()` / batch ops / `touch()` TTL extension | ✅ |
| Localization | `Translator` / `__()` / `trans_choice()` / pluralization / JSON files | ✅ |
| Testing | `TestClient` / `TestResponse` / `RefreshDatabase` | ✅ |
| Task Scheduling | `Schedule` / `ScheduleManager` / cron parser / `schedule:run` CLI | ✅ |
| Queue routing | `QueueManager::route()` / `routed_queue()` / central job→queue rules | ✅ |
| Pagination default | 25 per page (Laravel 13 default) | ✅ |
| JSON:API resources (relationship inclusion, sparse fieldsets, links) | `JsonApiResource` trait + `JsonApiItem` / `JsonApiCollection` — `?include=` compound documents, `?fields[type]=` sparse fieldsets, `when_included()`, `application/vnd.api+json` | ✅ |
| AI SDK foundation — text generation, streaming, structured output, embeddings | `Ai` facade + `AiProvider` trait — `generate()`, `chat()`, `chat_stream()`, `structured()`, `embed()` with 30-day caching, OpenAI-compatible HTTP provider, `FakeAi` for tests | ✅ |
| AI SDK agents / media (agents, images, audio, TTS/STT, vector stores, reranking) | `Ai::agent()` — persona prompt, `AgentTool` tool calling loop (JSON-schema tools, error recovery, turn limit), `AgentTask` / `AgentResult` / `AgentTaskStatus`; failover via `chat_with_fallback()` / `generate_with_fallback()`; `Media` value type; image create/edit/variation; TTS / STT; moderation; reranking (`Ai::rerank()`); vector stores (`FileVectorStore`, `PostgresVectorStore` + pgvector) | ✅ |

## Laravel 13 gaps (not yet implemented)

| Laravel 13 Feature | Larastvel Equivalent | Status |
|---|---|---|
| Laravel AI SDK (agents, embeddings, audio, images, vector stores) | `Ai` facade + agents with tool calling, failover, `Media`, image generation/editing/variation, TTS / STT, moderation, reranking, and vector stores (`FileVectorStore`, `PostgresVectorStore`) | ✅ |
| Semantic / vector search (`whereVectorSimilarTo()`, pgvector) | `VectorSimilarityQuery` — cosine / L2 / inner product on `Select<E>` | ✅ |
| `PreventRequestForgery` (origin-aware CSRF) | `Sec-Fetch-Site` origin verification, `allow_same_site()` / `use_origin_only()` | ✅ |
| Job attributes (`#[Tries]`, `#[Backoff]`, `#[Timeout]`, `#[FailOnTimeout]`) | `#[job(tries, backoff, timeout, fail_on_timeout)]` with worker retry, delay, timeout enforcement | ✅ |

## Deferred gaps (tracked, not yet implemented)

| Laravel 13 Feature | Larastvel Equivalent | Status |
|---|---|---|
| Blade components (`x-slot` slots) | not implemented — tracked in `scripts/parity-audit.sh` `DEFERRED_FEATURES` | 🕐 |
| Passkey authentication (WebAuthn) | not implemented — tracked in `scripts/parity-audit.sh` `DEFERRED_FEATURES` | 🕐 |
| Reverb database broadcasting driver | not implemented — tracked in `scripts/parity-audit.sh` `DEFERRED_FEATURES` | 🕐 |
| Redis cache store | not implemented — tracked in `scripts/parity-audit.sh` `DEFERRED_FEATURES` | 🕐 |

~100% feature parity with 1160+ unit tests (checked against Laravel 13.23.0).
