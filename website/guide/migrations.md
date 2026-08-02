# Migrations

Larastvel uses SeaORM's migration system for database schema management.

## Creating Migrations

```bash
larastvel make migration create_users_table
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
use larastvel_core::database::DatabaseSeeder;
use larastvel_core::models::factory::{factory_create, factory_create_count, Faker, ModelFactory};

// Faker is a unit struct — its methods are static
let name = Faker::name();
let email = Faker::email();

// Define a factory
#[derive(Default)]
struct UserFactory;

impl ModelFactory for UserFactory {
    type ActiveModel = user::ActiveModel;

    fn definition() -> Self::ActiveModel {
        use sea_orm::Set;
        user::ActiveModel {
            id: sea_orm::NotSet,
            name: Set(Faker::name()),
            email: Set(Faker::email()),
        }
    }
}

// Build in-memory instances (not persisted)
let draft = UserFactory::make();
let batch = UserFactory::make_count(10);

// Persist records
let user = factory_create::<UserFactory>().await?;
let users = factory_create_count::<UserFactory>(10).await?;

// Seed the database
DatabaseSeeder::run_seeder::<UserSeeder>(&conn).await?;
```
