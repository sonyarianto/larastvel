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
larastvel make command InspireCommand
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
larastvel make provider AppServiceProvider
```

## Conditional Bindings (`#[bind_when]`)

The container supports Laravel 13.22's `#[BindWhen]` conditional bindings:
which concrete instance a name resolves to is decided at resolve time based on
the live application config.

Register conditions imperatively with `bind_if_config` (config-key truthy),
`bind_if` (arbitrary closure), and a final `bind_default` fallback. Rules are
evaluated in declaration order and the first match wins:

```rust
use larastvel_core::foundation::Application;
use std::sync::Arc;

app.bind_if_config("payment-gateway", "features.payments.beta", "beta-gateway".to_string());
app.bind_default("payment-gateway", "fallback-gateway".to_string());

// Resolution picks the first condition matching live config:
let gateway: Option<String> = app.make_by_alias("payment-gateway");

// Gates an instance on arbitrary config, evaluated at resolve time:
app.bind_if("gateway", |config| config.get("app.env") == Some("production".to_string()), "prod".to_string());
```

The `#[bind_when]` attribute macro turns a trait into a self-registering
conditional binding that mirrors Laravel's attribute style. It generates a
companion `struct {Trait}ConditionalBindings` with `ALIAS` / `CONDITION_KEY`
constants, `conditional_enabled`, `register_conditional_binding`, and
`resolve`:

```rust
use larastvel_core::bind_when;
use larastvel_core::foundation::Application;
use std::sync::Arc;

#[bind_when(alias = "payment-gateway", condition_key = "features.payments.beta")]
trait PaymentGateway: Send + Sync + 'static {
    fn process(&self) -> String;
}

struct OptimizedGateway;
impl PaymentGateway for OptimizedGateway {
    fn process(&self) -> String { "optimized".into() }
}

// Wire the conditional implementations from a service provider:
PaymentGatewayConditionalBindings::register_conditional_binding(&app, || {
    Arc::new(OptimizedGateway) as Arc<dyn PaymentGateway>
});

// Config change flips which binding resolves:
let mut config = app.config();
config.set("features.payments.beta", "1");
app.set_config(config);

// Resolution returns the conditional instance when the key is truthy:
assert!(PaymentGatewayConditionalBindings::conditional_enabled(&app));
let gateway: Option<Arc<dyn PaymentGateway>> = PaymentGatewayConditionalBindings::resolve(&app);
```

`bind_if` / `bind_if_config` / `bind_default` are methods on `Application`
(`crates/larastvel-core/src/foundation`); `config_bool` drives the `#[bind_when]` generated gate.
