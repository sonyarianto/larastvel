# Database & ORM

Larastvel uses **SeaORM** as its ORM, providing an Eloquent-like experience in Rust.

## Configuration

Configure your database in `config/database.toml`:

```toml
driver = "sqlite"       # sqlite, postgres, mysql
host = "127.0.0.1"
port = 3306
database = "larastvel"
username = "root"
password = ""
```

## Models

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
}
```

## DbModel Trait

Larastvel extends SeaORM with a `DbModel` trait providing Laravel-style helpers:

```rust
let users = User::all().await?;
let user = User::find(1).await?;
User::create(user_data).await?;
User::update(1, update_data).await?;
User::delete(1).await?;
```

## Migrations

Generate and run migrations via CLI:

```bash
cargo run -p larastvel-cli -- make migration create_users_table
cargo run -p larastvel-cli -- migrate
```

## Seeders

```rust
#[derive(Seeder)]
struct UserSeeder;

impl Seeder for UserSeeder {
    async fn run(&self, db: &DatabaseConnection) -> Result<()> {
        // Seed data
    }
}
```

```bash
cargo run -p larastvel-cli -- db:seed
```

## Model Factories

```rust
factory_create::<User>(UserFactory, 10).await?;
```

Uses Faker for realistic test data.
