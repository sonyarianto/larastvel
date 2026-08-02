# AGENTS.md

Guidance for AI agents and contributors working on the Larastvel codebase.

## Project Overview

Larastvel is a Rust web framework inspired by Laravel, built on Axum, Tokio,
and SeaORM. It tracks **Laravel 13 (baseline v13.23.0)** feature parity — the
parity status lives in `PARITY.md` at the repo root.

- The workspace root (`src/`) is the sample application `larastvel-app`.
- `crates/larastvel-core` is the framework itself. Everything else is support.

## Workspace Layout

| Crate | Purpose |
|---|---|
| `crates/larastvel-core` | Framework core — routing, ORM, config, auth, sessions, caching, queues, broadcasting, AI, JSON:API, macros re-exports |
| `crates/larastvel-macros` | Procedural macros (`#[route]`, `#[job]`, `#[table]`, `#[policy]`, `#[factory]`, `#[seeder]`, `#[command]`, `#[api_resource]`, `#[json_api_resource]`, `#[provider]`, `#[broadcast_event]`, `#[mail]`, …) |
| `crates/larastvel-cli` | Artisan-like CLI (`larastvel` binary) — commands live in `src/commands/` |
| `crates/larastvel-testing` | Test utilities (`TestClient`, `TestResponse`, `RefreshDatabase`) — NOT part of core |
| `crates/larastvel-tinker` | Interactive REPL |
| `crates/larastvel-new` | Project scaffolding generator (`larastvel-new` binary) |

## Quality Gates — always run before committing

Lefthook runs these on `git commit` (see `lefthook.yml`):

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

Full verification (also what CI runs):

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace   # ~1075 tests, all must pass
```

If the VitePress docs changed, also verify the site builds:

```bash
cd website && npm run build
```

## Code Conventions

- Follow the existing module layout (`crates/larastvel-core/src/<feature>/`)
  and mimic the style of neighboring code.
- Public API mirrors Laravel 13 where possible — **snake_case** methods
  (`assert_dispatched`, not `assertDispatched`).
- Do NOT add code comments unless the user asks. Rustdoc (`///`) on public
  items is expected.
- Re-export new public APIs from `crates/larastvel-core/src/lib.rs`.
- Tests are written inside the same file (`#[cfg(test)] mod tests`), using
  in-process mock servers (`tokio::net::TcpListener`) for HTTP behavior —
  never hit real network endpoints.
- Core is a single crate; adding dependencies requires checking the
  existing set first (tokio, axum 0.8, sea-orm 1.1.x, reqwest 0.13, tokio,
  serde, futures-util, sha2, base64, hex, toml).

## Docs Maintenance

`website/` is VitePress docs; `PARITY.md` and `CHANGELOG.md` live at root.

- **Drift rule:** every code example in the docs must be verified against the
  actual source. When APIs change, sweep `website/guide/*.md`,
  `website/reference/*.md`, `README.md`, and `CHANGELOG.md` for stale symbols
  (this has bitten the project before — see the v0.2.1 docs drift sweep).
- Sidebar entries are in `website/.vitepress/config.mts`.
- The version shown in the docs nav (`v0.2.1`) is updated on every release —
  see `website/reference/versions.md`.

## Release Process (v0.2.1+)

1. Bump `version = "0.2.1"` → `0.3.0` in **all 7 manifests**:
   `Cargo.toml` (root) and `crates/*/Cargo.toml`. Keep cross-crate deps as
   `version = "0.x"` (semver-compatible) unless a breaking change requires
   bumping them.
2. Add a `CHANGELOG.md` section for the new version (date = today).
3. Update version references: `website/reference/versions.md`,
   `website/.vitepress/config.mts` nav badge, `website/guide/getting-started.md`.
   README badge is dynamic (crates.io) — no change needed.
4. Run quality gates above, then commit and push.
5. Tag the release: `git tag v0.3.0 && git push origin v0.3.0`.
 6. **Publishing happens automatically** via `.github/workflows/publish.yml`
    when the tag is pushed — it verifies the tag matches all crate versions,
    then publishes in dependency order:
    `larastvel-macros` → `larastvel-core` → `larastvel-testing` /
    `larastvel-tinker` / `larastvel-cli` → `larastvel-new`.
    The workflow is idempotent: already-published versions are skipped and
    index propagation is awaited, so a partial failure can be resumed with
    `gh workflow run publish.yml --ref vX.Y.Z`.
 7. After every successful publish, `.github/workflows/scaffold-check.yml`
    builds a fresh scaffolded project against the just-published crates (also
    weekly and on manual dispatch) — an automatic guard for scaffold drift.
 8. Requires the `CARGO_REGISTRY_TOKEN` GitHub secret — set via
    `gh secret set CARGO_REGISTRY_TOKEN`.

Do not run `cargo publish` locally for releases; the CI workflow owns
publishing.

## Gotchas

- **SeaORM 2.0**: the workspace is on `sea-orm 1.1.20` + `sea-orm-migration
  1.1.20`. SeaORM 2.0 was released (2026-07-19) but the migration is
  deliberately deferred — tracked in GitHub issue #1
  (https://github.com/sonyarianto/larastvel/issues/1). Do not bump SeaORM
  without addressing that issue.
- `#[json_api_resource]` / `#[api_resource]` macros are attribute macros in
  `larastvel-macros`; both are re-exported from core.
- `crates/larastvel-cli` and `crates/larastvel-new` both generate project
  scaffolds — they must stay in sync (the CLI scaffold regressed once and
  was re-aligned with `larastvel-new` in v0.2.1). The `scaffold-check`
  workflow is the automated guard: keep it green when changing scaffolds.
- The CLI uses clap subcommands: `larastvel make migration` (space form) —
  NOT `make:migration` (that's the hand-written help text style only).
- **Cross-crate dependencies must use `{ path = "../<crate>", version = "0.x" }`**
  (never a bare `version`): `cargo publish` verification copies the workspace
  `Cargo.lock` into the package, and a bare version dep resolves the stale
  registry version from the lock, breaking publish (hit at v0.2.1).
