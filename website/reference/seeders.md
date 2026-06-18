# Seeders

The `#[seeder]` attribute macro generates a `Seeder` trait implementation for populating databases with test data.

## Usage

```rust
use larastvel_core::sea_orm::DbConn;

#[seeder]
struct UserSeeder;

impl UserSeeder {
    async fn seed(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
        use sea_orm::ActiveValue::Set;
        // Insert seed data
        Ok(())
    }
}
```

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string literal | no | Custom seeder name (defaults to PascalCase → snake_case of struct name) |

With a custom name:

```rust
#[seeder("custom_users")]
struct UserSeeder;
```

## Generated Implementation

The macro generates:

```rust
#[larastvel_core::async_trait]
impl Seeder for UserSeeder {
    fn name(&self) -> &'static str {
        "user_seeder" // or custom name
    }

    async fn run(&self, conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
        Self::seed(conn).await
    }
}
```

The `#[larastvel_core::async_trait]` annotation is added to the generated impl block to handle lifetime elision correctly with the async trait method.

## User Method

Your struct must define a `seed` associated function (name chosen to avoid collision with `Seeder::run`):

```rust
async fn seed(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>>
```

## Running Seeders

```bash
cargo run -p larastvel-cli -- db:seed
```

## CLI Generator

```bash
larastvel make:seeder UserSeeder
```
