# Architecture

## Overview

```
┌──────────────────────────────────────────────────────┐
│                     Application                        │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │  Config  │  │    DB    │  │  Service Container │  │
│  │  (TOML)  │  │ (SeaORM) │  │  (TypeId-based)    │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
│  ┌────────────────────────────────────────────────┐  │
│  │           Router (Axum + Registrar)            │  │
│  │    Routes → Groups → Middleware → Controllers  │  │
│  └────────────────────────────────────────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │ Session  │  │  Cache   │  │  Queue / Events    │  │
│  │ + CSRF   │  │ (stores) │  │  + Notifications   │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

## Crate Layout

The project is a Cargo workspace with 7 crates:

| Crate | Purpose |
|---|---|
| `larastvel-core` | Framework core — router, DB, config, views, middleware |
| `larastvel-cli` | Artisan-like CLI binary |
| `larastvel-macros` | Procedural macros (`Resource`, `api_resource`, `controller`, `route`, `command`, `table`, `job`, `scope`, `observer`, `notification`, `rule`, `policy`, `provider`, `seeder`, `factory`) |
| `larastvel-tinker` | Interactive REPL binary |
| `larastvel-new` | Project scaffolding binary |
| `larastvel-testing` | Test utilities (`TestClient`, `TestResponse`, `RefreshDatabase`) |
| `larastvel-app` (root) | Application entrypoint |

## Request Lifecycle

1. **HTTP request** arrives at Axum server
2. **SessionLayer** decrypts cookies, loads session data
3. **CsrfLayer** validates CSRF tokens (excluded for API/health routes)
4. **User middleware** runs (auth, rate limiting, etc.)
5. **Router** matches route → calls handler
6. **Handler** returns response (possibly via Tera view)
7. **Response** sent back through middleware layers in reverse

## Application Builder

```rust
App::new()
    .config(Config::load())
    .database(DatabaseManager::new(&config))
    .registrar(|r| {
        web(&r);
        api(&r);
    })
    .with_layer(MyCustomLayer)
    .run()
    .await
```

Session and CSRF layers are auto-wired when `app.key` is present in config.

## Custom Commands

The `#[command]` attribute macro generates a `Command` trait implementation for Artisan-style CLI commands. See the [full reference](/reference/commands) for details, arguments, and generated code.

```rust
use larastvel_core::console::{Command, ConsoleError};
use larastvel_core::foundation::Application;

#[command("inspire", description = "Display an inspiring quote")]
#[derive(Debug)]
struct InspireCommand;

impl InspireCommand {
    fn run(&self, _app: &Application, _args: &[String]) -> Result<(), ConsoleError> {
        println!("Simplicity is the ultimate sophistication.");
        Ok(())
    }
}
```

Generate a scaffolded command with:

```bash
larastvel make:command InspireCommand
```

## Service Providers

The `#[provider]` attribute macro generates a `ServiceProvider` implementation. See the [full reference](/reference/providers) for details.

```rust
use larastvel_core::foundation::Application;
use larastvel_core::provider;

#[provider]
struct AppServiceProvider;

impl AppServiceProvider {
    fn register_services(&self, app: &Application) {
        app.bind(MyService::new());
    }
}
```

Generate a scaffolded provider with:

```bash
larastvel make:provider AppServiceProvider
```
