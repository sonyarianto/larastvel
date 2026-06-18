# CLI Reference

Larastvel ships with an Artisan-like CLI.

## Commands

| Command | Description |
|---|---|
| `serve` | Start the development server |
| `key:generate` | Generate a 32-byte base64 encryption key |
| `route:list` | List all registered routes |
| `route:cache` | Cache routes for faster resolution |
| `route:clear` | Clear cached routes |
| `config:cache` | Cache configuration |
| `config:clear` | Clear cached configuration |
| `migrate` | Run pending database migrations |
| `make:model` | Create a new model |
| `make:controller` | Create a new controller |
| `make:migration` | Create a new migration |
| `make:seeder` | Create a new seeder |
| `make:policy` | Create a new policy |
| `make:test` | Create a new test |
| `make:job` | Create a new job |
| `make:event` | Create a new event |
| `make:notification` | Create a new notification |
| `make:rule` | Create a new validation rule |
| `make:mail` | Create a new mail class |
| `make:command` | Create a new CLI command |
| `make:resource` | Create a new API resource |
| `make:provider` | Create a new service provider |
| `db:seed` | Seed the database |
| `notifications:table` | Create notifications migration |
| `storage:link` | Create a symbolic link from public/storage to storage/app/public |
| `schedule:list` | List scheduled tasks |
| `schedule:run` | Run due scheduled tasks |
| `queue:work` | Start processing queued jobs |
| `version` | Display framework version |

## Usage

```bash
# Run via cargo
cargo run -p larastvel-cli -- serve
cargo run -p larastvel-cli -- key:generate
cargo run -p larastvel-cli -- make model User
cargo run -p larastvel-cli -- route:list
cargo run -p larastvel-cli -- migrate
cargo run -p larastvel-cli -- queue:work

# Or after installation
larastvel serve
larastvel make:controller PostController
```

## Dev Server

The `serve` command starts the Axum server with auto-reload support:

```bash
cargo run -p larastvel-cli -- serve
# Listening on http://localhost:8080
```

## Global Installation

```bash
cargo install larastvel-cli
larastvel serve
larastvel make:controller PostController
```
