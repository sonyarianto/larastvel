# Service Providers

The `#[provider]` attribute macro generates a `ServiceProvider` trait implementation for registering services into the application container.

## Usage

```rust
use larastvel_core::foundation::Application;
use larastvel_core::provider;

#[provider]
struct AppServiceProvider;

impl AppServiceProvider {
    fn register_services(&self, app: &Application) {
        // Bind services into the container
        app.bind(MyService::new());
    }
}
```

## Arguments

This macro takes no arguments — just `#[provider]` on the struct.

## Generated Implementation

The macro generates:

```rust
impl ServiceProvider for AppServiceProvider {
    fn register(&self, app: &Application) {
        self.register_services(app);
    }
}
```

The `boot()` and `provides()` methods use the trait's default implementations (no-op and empty, respectively).

## User Method

Your struct must define a `register_services` method (name chosen to avoid collision with `ServiceProvider::register`):

```rust
fn register_services(&self, app: &Application)
```

## Custom boot() or provides()

If you need custom boot or provides logic, implement the trait manually instead of using the macro:

```rust
impl ServiceProvider for AppServiceProvider {
    fn register(&self, app: &Application) {
        self.register_services(app);
    }

    fn boot(&self, app: &Application) {
        // Custom boot logic
    }

    fn provides(&self) -> Vec<&'static str> {
        vec!["my-service"]
    }
}
```

## CLI Generator

```bash
larastvel make:provider AppServiceProvider
```
