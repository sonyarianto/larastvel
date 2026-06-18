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
use larastvel_core::table;

#[table("users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
}
```

The `#[table]` macro generates the full SeaORM entity boilerplate (`DeriveEntityModel`, `Relation`, `ActiveModelBehavior`) plus a `DbModel` wrapper automatically.

## DbModel Trait

The `DbModel` trait provides Laravel-style helpers on top of SeaORM entities:

```rust
let users = User::all().await?;
let user = User::find(1).await?;
User::create(user_data).await?;
User::update(1, update_data).await?;
User::delete(1).await?;
```

## Query Scopes

Use the `#[scope]` macro to define reusable query constraints on your models. The scope function receives a `Select<Entity>` as its first parameter (removed from the public API) and returns a modified query:

```rust
use larastvel_core::scope;

impl User {
    /// Find users with at least N followers.
    #[scope]
    fn popular(query: Select<Entity>, min_followers: i64) -> Select<Entity> {
        query.filter(Column::Followers.gte(min_followers))
    }
}
```

The generated method chains onto `Self::query()` automatically:

```rust
let users = User::popular(100).all().await?;
```

Laravel's `scope_` prefix convention is supported — `scope_popular` becomes `popular()`:

```rust
impl User {
    #[scope]
    fn scope_recent(query: Select<Entity>, days: i64) -> Select<Entity> {
        query.filter(Column::CreatedAt.gte(chrono::Utc::now().naive_utc() - chrono::Duration::days(days)))
    }
}

// Call without the scope_ prefix:
let users = User::recent(7).all().await?;
```

Generate a scaffolded scope with:

```bash
larastvel make:scope popular
```

## Model Observers

Observers allow you to hook into model lifecycle events — `created`, `updated`, `deleted`, `saved`, and `retrieved` — by defining handler methods on a dedicated struct.

```rust
use larastvel_core::observer;

struct UserObserver;

#[observer(User)]
impl UserObserver {
    async fn created(&self, user: Model) {
        // React to new user creation
    }

    async fn deleted(&self, user: Model) {
        // React to user deletion
    }
}

// Register the observer at app boot:
UserObserver::observe();
```

Only the hook methods you define are wired up — if you omit `updated`, no `ModelUpdated` listener is registered.

The `DbModel` trait automatically dispatches these events:
- `find()` → `ModelRetrieved`
- `insert()` → `ModelCreated` + `ModelSaved`
- `update()` → `ModelUpdated` + `ModelSaved`
- `delete()` → `ModelDeleted`

Generate a scaffolded observer with:

```bash
larastvel make:observer UserObserver
```

## Migrations

Generate and run migrations via CLI:

```bash
cargo run -p larastvel-cli -- make migration create_users_table
cargo run -p larastvel-cli -- migrate
```

## Seeders

The `#[seeder]` macro generates a `Seeder` trait implementation. See the [full reference](/reference/seeders) for details, arguments, and generated code.

```rust
use larastvel_core::sea_orm::DbConn;

#[seeder]
struct UserSeeder;

impl UserSeeder {
    async fn seed(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
        // Insert seed data
        Ok(())
    }
}
```

Run seeders:

```bash
cargo run -p larastvel-cli -- db:seed
```

## Model Factories

The `#[factory]` macro generates a `ModelFactory` trait implementation. See the [full reference](/reference/factories) for details.

```rust
use larastvel_core::models::factory::Faker;
use larastvel_core::sea_orm::Set;
use sea_orm::entity::prelude::*;

#[derive(Debug, Default)]
#[factory("user")]
pub struct UserFactory;

impl UserFactory {
    fn define() -> crate::models::user::ActiveModel {
        user::ActiveModel {
            name: Set(Faker::name()),
            email: Set(Faker::email()),
            ..Default::default()
        }
    }
}
```

```rust
factory_create::<UserFactory>().await?;
factory_create_count::<UserFactory>(10).await?;
```

## API Resources

The `#[api_resource]` macro generates an `ApiResource` trait implementation. See the [full reference](/reference/api-resources) for details, including how to use single-model and collection transforms.

```rust
use larastvel_core::api_resource;

#[api_resource(crate::models::user::Model)]
#[derive(Debug)]
struct UserResource;

impl UserResource {
    fn to_array(model: &crate::models::user::Model) -> serde_json::Value {
        serde_json::json!({
            "id": model.id,
            "name": model.name,
            "email": model.email,
        })
    }
}
```

```rust
let resource = UserResource::make(user);
let json = resource.value();

let collection = UserResource::collect(users);
let json = collection.value();
```

Generate a scaffolded resource with:

```bash
larastvel make:resource UserResource
```
