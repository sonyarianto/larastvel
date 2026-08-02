# Releases

## Current version: 0.2.2

The latest release of all Larastvel crates is **v0.2.2** (2026-08-02). Install it with:

```bash
cargo install larastvel-new
cargo install larastvel-cli
```

Or add the framework to your project:

```bash
cargo add larastvel-core
```

## Release history

| Version | Date | Highlights |
|---|---|---|
| **v0.2.2** | 2026-08-02 | First-party image processing (`Image` facade — resize/cover/crop pipeline, outputs, `storage/app`, test fake), container conditional bindings (`#[bind_when]` — Laravel 13.22 `#[BindWhen]` parity), `email:dns` validation rule, `CookieJar` with `(name, path)`-keyed `queued()` (Laravel 13.24 fix), scheduling timezones + `next_run()` |
| **v0.2.1** | 2026-08-02 | AI SDK foundation (`Ai`, streaming, structured output, embeddings with caching, `FakeAi`), JSON:API resources (compound documents, sparse fieldsets), vector search (pgvector), origin-aware CSRF, queue routing + job attributes, `Cache::touch()`, 16+ new attribute macros (`#[route]`, `#[job]`, `#[table]`, `#[policy]`, `#[factory]`, `#[seeder]`, `#[command]`, `#[api_resource]`, `#[provider]`, `#[broadcast_event]`, `#[mail]`, …), `Pipeline`, `Str`/`Arr`/`Number`/`Stringable` helpers |
| v0.2.0 | 2026-06-14 | First crates.io release — all 6 workspace crates: `larastvel-core`, `larastvel-cli`, `larastvel-macros`, `larastvel-testing`, `larastvel-tinker`, `larastvel-new` |
| v0.1.0 | — | Initial development — routing, ORM, auth, sessions, caching, artisan-like CLI |

## Crates

| Crate | Description | crates.io |
|---|---|---|
| `larastvel-core` | Framework core — routing, ORM, config, auth, sessions, caching, queues, broadcasting, AI | [![crates.io](https://img.shields.io/crates/v/larastvel-core.svg)](https://crates.io/crates/larastvel-core) |
| `larastvel-macros` | Procedural macros | [![crates.io](https://img.shields.io/crates/v/larastvel-macros.svg)](https://crates.io/crates/larastvel-macros) |
| `larastvel-cli` | Artisan-like CLI (`larastvel` command) | [![crates.io](https://img.shields.io/crates/v/larastvel-cli.svg)](https://crates.io/crates/larastvel-cli) |
| `larastvel-testing` | Test utilities (`TestClient`, `TestResponse`, `RefreshDatabase`) | [![crates.io](https://img.shields.io/crates/v/larastvel-testing.svg)](https://crates.io/crates/larastvel-testing) |
| `larastvel-tinker` | Interactive REPL | [![crates.io](https://img.shields.io/crates/v/larastvel-tinker.svg)](https://crates.io/crates/larastvel-tinker) |
| `larastvel-new` | Project scaffolding (`larastvel-new` command) | [![crates.io](https://img.shields.io/crates/v/larastvel-new.svg)](https://crates.io/crates/larastvel-new) |

## Changelog

The full changelog is maintained in the repository: [CHANGELOG.md](https://github.com/sonyarianto/larastvel/blob/main/CHANGELOG.md)
