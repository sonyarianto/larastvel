# Changelog

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
- 611+ unit tests across the workspace
- CI: `cargo fmt --check` → `clippy` → `build` → `test` on push/PR to main

## v0.1.0 (Initial development)

- Initial framework scaffolding with routing, ORM, auth, sessions, caching
- Artisan-like CLI with 25+ commands
- Blade-style Tera directives (`@auth`, `@csrf`, `@error`, `@guest`, `@method`)
- Laravel feature parity
