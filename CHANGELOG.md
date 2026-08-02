# Changelog

## Unreleased

### ✨ New
- **First-party image processing** (`Image` facade, Laravel 13.20 parity): `Image::from_bytes/base64/path/url/storage`, immutable `ImageInstance` pipeline (`resize`/`scale`/`cover`/`crop`/`contain`/`rotate`/`grayscale`/`blur`/`sharpen`/`flip`/`orient`), outputs (`to_png/jpg/webp/gif/bmp`, `to_base64`, `to_data_uri`), `save`/`store`/`store_publicly` under `storage/app`, `dominant_color`/`dimensions`, and test fake `Image::fake()` + `assert_resized`/`assert_covered`/`assert_cropped`/`assert_stored`…
- **Container conditional bindings** (`#[BindWhen]`, Laravel 13.22 parity): `Application::bind_if` / `bind_if_config` / `bind_default`, `Config::set`, `Application::config_bool`, and the `#[bind_when]` attribute macro generating a `<Trait>ConditionalBindings` registrar
- **`email:dns` validation rule** (Laravel 13.22 parity) — format + real DNS check, skipped under `fake_dns_lookups(true)`
- **CookieJar**: `Cookie` / `CookieJar` with `(name, path)`-keyed `queued()` (Laravel 13.24 fix), `queue`/`unqueue`/`forget`/`to_set_cookie_headers`
- **Scheduling polish**: `ScheduledEvent::timezone()` (IANA) honored by `is_due`, new `next_run()` for `schedule:list`

### 🐛 Fixed
- EXIF orientation is not decodable by the `image` crate's decoders — `orient` stays in the pipeline but decodes without EXIF metadata
- CookieJar queued cookies clobbered when the same name was queued on different paths — now keyed by `(name, path)`

## v0.2.1 (2026-08-02)

### ✨ New
- **AI SDK foundation** (Laravel `laravel/ai` parity, Phase 1): `Ai` facade with `generate()`, `chat()`, `chat_stream()`, `structured()`, `embed()`/`embed_many()`; `AiProvider` trait; OpenAI-compatible HTTP provider (chat completions, SSE streaming, embeddings); embeddings cached for 30 days via `CacheManager`; `FakeAi` for tests
- **JSON:API resources**: `JsonApiResource` trait + `JsonApiItem`/`JsonApiCollection` — `?include=` compound documents, `?fields[type]=` sparse fieldsets, `when_included()`/`when_not_included()`, `application/vnd.api+json`, `#[json_api_resource]` macro
- **Vector search** (pgvector): `VectorSimilarityQuery` with cosine / L2 / inner-product operators
- **Origin-aware CSRF** (`PreventRequestForgery` parity): `Sec-Fetch-Site` origin verification, `allow_same_site()` / `use_origin_only()`
- **Queue routing**: `QueueManager::route()` / `unroute()`, and job attributes `#[job(tries, backoff, timeout, fail_on_timeout)]` enforced by the worker
- **`Cache::touch()`**; pagination default raised to 25
- **16 new attribute macros**: `#[route]`, `#[can]`, `#[table]`, `#[job]`, `#[queued_listener]`, `#[scope]`, `#[observer]`, `#[notification]`, `#[rule]`, `#[command]`, `#[policy]`, `#[seeder]`, `#[factory]`, `#[api_resource]`, `#[provider]`, `#[broadcast_event]`, `#[mail]`
- **`Pipeline`** data transformation workflows; `Str`, `Arr`, `Number`, `Stringable` helpers; Prompt CLI helpers; criterion benchmarks

### 🔧 Changed
- `Config::get()` now resolves dotted paths through nested config sections (e.g. `ai.provider`)
- CLI make-target help and docs use `make <target>` (clap subcommand form) instead of `make:<target>`
- `larastvel new` scaffold: routes + database modules generated under `src/`, routes wired into the router, migrations run at boot

### 🐛 Fixed
- `larastvel new` scaffold did not compile — `mod routes;` pointed at a root-level `routes/` directory that was never a Rust module; unused imports removed
- Generated projects now compile against the current API (requires republish — this release)
- `proc-macro-error2` future-incompat warning dropped (`sea-bae 0.2.2`)
- Docs drift sweep: 28 files — every code example verified against the actual source (auth, authorization, broadcasting, caching, database, encryption, errors, localization, migrations, pagination, passwords, pipeline, queues, rate-limiting, routing, scheduling, session, sms, testing, views, seeders, policies, CLI reference, parity)

### 📚 Docs
- New JSON:API resources reference page
- Website parity sweep: pagination, queues (job attributes + routing), session (origin verification), caching guides updated to the real APIs

## v0.2.0 (2026-06-14)

### 🚀 First crates.io release

All 6 workspace crates published to crates.io.

- **larastvel-core** — Framework core: routing, ORM, config, auth, sessions, caching, queues, broadcasting, and more
- **larastvel-cli** — Artisan-like CLI binary (`larastvel` command)
- **larastvel-macros** — Procedural macros (`Resource`, `controller`, `route`)
- **larastvel-testing** — Test utilities (`TestClient`, `TestResponse`, `RefreshDatabase`)
- **larastvel-tinker** — Interactive REPL binary
- **larastvel-new** — Project scaffolding binary (`cargo install larastvel-new`)

### ✨ New
- Config directory support: `config/*.toml` with fallback to legacy `config.toml`
- Session + CSRF middleware auto-wired when `app.key` is configured
- `App::with_layer()` for custom middleware layers
- CSRF middleware with header and form-field token validation (constant-time)
- Dark mode default for documentation website
- `llms.txt` for LLM-agent consumption
- CRUD operations — create, read, update, delete ([#3](https://github.com/sonyarianto/larastvel/issues/3))

### 🔧 Changed
- All crate versions bumped to v0.2.0 consistently
- Workspace path dependencies migrated to version dependencies for publishing
- `larastvel-core` re-exports `axum` as `pub use axum`
- CLI `env` command shows `config/` directory or legacy `config.toml`
- Scaffold generates `config/*.toml` per-section files
- Features table sorted alphabetically

### 🐛 Fixed
- Scaffold template: route files use `larastvel_core::axum` instead of bare `axum`
- Scaffold template: module/function name collision in `routes::web` / `routes::api`
- Scaffold template: unused `Config` and `Registrar` imports removed
- ASCII architecture diagram box alignment
- VitePress base path for Vercel deployment (`/` vs `/larastvel/`)

### 📚 Docs
- Official documentation site at [larastvel.vercel.app](https://larastvel.vercel.app)
- Landing page, 9 guide pages, CLI reference, parity tracking
- README revamp with badges, feature table, config reference, architecture diagram
- `PARITY.md` extracted from README

### 🧪 Testing
- 1000+ unit tests across the workspace
- CI: `cargo fmt --check` → `clippy` → `build` → `test` on push/PR to main

## v0.1.0 (Initial development)

- Initial framework scaffolding with routing, ORM, auth, sessions, caching
- Artisan-like CLI with 25+ commands
- Blade-style Tera directives (`@auth`, `@csrf`, `@error`, `@guest`, `@method`)
- Laravel feature parity
