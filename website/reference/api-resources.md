# API Resources

The `#[api_resource]` attribute macro generates an `ApiResource` trait implementation for transforming models into JSON API output.

## Usage

```rust
use larastvel_core::api_resource;
use serde::Serialize;

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

## Arguments

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `model` | type | yes | The model type to transform (e.g., `crate::models::user::Model`, `self::Product`) |

Supports any valid Rust type path — single ident (`Product`) or module path (`crate::models::user::Model`).

## Generated Implementation

The macro generates:

```rust
impl ApiResource<crate::models::user::Model> for UserResource {
    fn transform(model: &crate::models::user::Model) -> serde_json::Value {
        Self::to_array(model)
    }
}
```

## User Method

Your struct must define a `to_array` associated function (name chosen to avoid collision with `ApiResource::transform`):

```rust
fn to_array(model: &ModelType) -> serde_json::Value
```

## Usage

```rust
// Single model
let resource = UserResource::make(user);
let json = resource.value();

// Collection
let collection = UserResource::collect(users);
let json = collection.value();
```

## CLI Generator

```bash
larastvel make:resource UserResource
```
