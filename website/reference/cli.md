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
| `route:conflicts` | Detect overlapping route definitions (reads the routes cache) |
| `config:cache` | Cache configuration |
| `config:clear` | Clear cached configuration |
| `migrate` | Run pending database migrations |
| `make model` | Create a new model |
| `make controller` | Create a new controller |
| `make migration` | Create a new migration |
| `make seeder` | Create a new seeder |
| `make policy` | Create a new policy |
| `make test` | Create a new test |
| `make job` | Create a new job |
| `make event` | Create a new event |
| `make listener` | Create a new listener |
| `make notification` | Create a new notification |
| `make rule` | Create a new validation rule |
| `make command` | Create a new CLI command |
| `make factory` | Create a new factory |
| `make mail` | Create a new mail class |
| `make scope` | Create a new query scope |
| `make observer` | Create a new model observer |
| `make resource` | Create a new API resource |
| `make provider` | Create a new service provider |
| `make broadcast-event` | Create a new broadcast event |
| `db:seed` | Seed the database |
| `notifications:table` | Create notifications migration |
| `storage:link` | Create a symbolic link from public/storage to storage/app/public |
| `schedule:list` | List scheduled tasks |
| `schedule:run` | Run due scheduled tasks |
| `queue:work` | Start processing queued jobs |
| `queue:failed` | List all failed jobs |
| `queue:retry` | Retry a failed job by id, or `all` |
| `queue:forget` | Forget a failed job by id |
| `queue:flush` | Forget all failed jobs |
| `about` | Display framework and environment information |
| `optimize` | Cache config and routes for faster boot |
| `optimize:clear` | Clear the config and route caches |
| `config:show` | Display the values of a config section (e.g. `config:show app`) |
| `down` | Put the application into maintenance mode (writes `storage/framework/down`) |
| `up` | Bring the application out of maintenance mode |
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
larastvel make controller PostController
```

## Dev Server

The `serve` command starts the Axum server with auto-reload support:

```bash
cargo run -p larastvel-cli -- serve
# Listening on http://localhost:8080
```

## Maintenance Mode

`down` writes `storage/framework/down` to enable maintenance mode; `up`
removes it. A secret enables the timing-safe bypass (Laravel 13.23 parity —
comparison is constant-time, no `==`):

```bash
larastvel down --with-secret
# Application is now in maintenance mode.
# You may bypass maintenance mode via [/RANDOM-SECRET].

larastvel down --secret my-secret
larastvel up
```

Bypass requests are compared in constant time against the stored secret via
`MaintenanceMode::bypass_secret_matches(path)` (`hash_equals`-style, Laravel
13.23 PR #60896).

## Global Installation

```bash
cargo install larastvel-cli
larastvel serve
larastvel make controller PostController
```
