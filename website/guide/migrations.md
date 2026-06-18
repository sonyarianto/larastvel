# Migrations

Larastvel uses SeaORM's migration system for database schema management.

## Creating Migrations

```bash
larastvel make:migration create_users_table
```

## Writing Migrations

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.create_table(
            Table::create()
                .table(Users::Table)
                .col(ColumnDef::new(Users::Id).integer().not_null().auto_increment().primary_key())
                .col(ColumnDef::new(Users::Name).string().not_null())
                .col(ColumnDef::new(Users::Email).string().not_null().unique_key())
                .col(ColumnDef::new(Users::CreatedAt).timestamp().default(Expr::current_timestamp()))
                .to_owned(),
        ).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await
    }
}
```

## Running Migrations

```bash
larastvel migrate         # run all pending
larastvel migrate:fresh   # drop all tables and re-run
larastvel migrate:rollback # rollback last batch
```

## Factories & Seeding

```rust
use larastvel_core::models::factory::{Faker, ModelFactory};

let mut faker = Faker::new();

// Generate fake data
let name = faker.name();
let email = faker.email();

// Define a factory
factory.define(User::default(), |faker| {
    User {
        name: faker.name(),
        email: faker.email(),
    }
});

// Create records
factory.create(User::default(), 10)?;

// Seed the database
seeder.call(UserSeeder).await?;
```
