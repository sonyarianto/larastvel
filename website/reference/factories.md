# Model Factories

The `#[factory]` attribute macro generates a `ModelFactory` trait implementation for creating model instances with fake data.

## Usage

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

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `model` | string literal | yes | Model name (lowercase, used to construct `crate::models::{model}::ActiveModel`) |

## Generated Implementation

The macro generates:

```rust
impl ModelFactory for UserFactory {
    type ActiveModel = crate::models::user::ActiveModel;

    fn definition() -> Self::ActiveModel {
        Self::define()
    }
}
```

The `ActiveModel` associated type is hardcoded to `crate::models::{model}::ActiveModel` — your models must live at that standard path.

## User Method

Your struct must define a `define` associated function (name chosen to avoid collision with `ModelFactory::definition`):

```rust
fn define() -> crate::models::user::ActiveModel
```

## Usage

```rust
// Create and persist a single model
factory_create::<UserFactory>().await?;

// Create multiple models
factory_create_count::<UserFactory>(10).await?;
```

## CLI Generator

```bash
larastvel make:factory UserFactory
```
