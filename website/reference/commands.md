# Commands

The `#[command]` attribute macro generates a `Command` trait implementation for Artisan-style CLI commands.

## Usage

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

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string literal | yes | Command name (used to invoke it) |
| `description` | named string param | no | Command description for help output |

## Generated Implementation

The macro generates:

```rust
impl Command for InspireCommand {
    fn name(&self) -> &'static str {
        "inspire"
    }

    fn description(&self) -> &'static str {
        "Display an inspiring quote"
    }

    fn handle(&self, app: &Application, args: &[String]) -> Result<(), ConsoleError> {
        self.run(app, args)
    }
}
```

## User Method

Your struct must define a `run` method (name chosen to avoid collision with `Command::handle`):

```rust
fn run(&self, app: &Application, args: &[String]) -> Result<(), ConsoleError>
```

## CLI Generator

```bash
larastvel make:command YourCommand
```

This scaffolds a command struct with the `#[command]` attribute and a placeholder `run()` method.
