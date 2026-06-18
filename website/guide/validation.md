# Validation

Larastvel provides a Laravel-inspired validation system with 20+ built-in rules.

## Basic Usage

```rust
use larastvel_core::validation::{validate, rules};
use std::collections::HashMap;
use serde_json::json;

let mut data = HashMap::new();
data.insert("email".to_string(), json!("user@example.com"));
data.insert("name".to_string(), json!("John"));

let result = validate(&data, vec![
    ("email", vec![rules::required(), rules::email()]),
    ("name", vec![rules::required(), rules::min(2), rules::max(50)]),
]);

match result {
    Ok(()) => { /* valid */ }
    Err(errors) => {
        // errors.has("email")
        // errors.first("email")
        // errors.to_json()
    }
}
```

## Available Rules

| Rule | Description |
|------|-------------|
| `required()` | Field must be present and non-empty |
| `email()` | Must be a valid email |
| `min(n)` | Minimum string length |
| `max(n)` | Maximum string length |
| `between(a, b)` | Length between a and b |
| `string()` | Must be a string |
| `numeric()` | Must be a number |
| `boolean()` | Must be a boolean |
| `alpha()` | Must contain only letters |
| `alpha_numeric()` | Must contain only letters/numbers |
| `url()` | Must be a valid URL |
| `ip()` | Must be a valid IP address |
| `confirmed()` | Field must match `field_confirmation` |
| `same(field)` | Must match another field |
| `different(field)` | Must differ from another field |
| `size(n)` | Exact length |
| `present()` | Field must exist (can be null/empty) |
| `prohibited()` | Field must be absent |
| `min_value(n)` | Numeric minimum |
| `max_value(n)` | Numeric maximum |
| `regex(pattern)` | Must match regex pattern |

## Attribute Macro Validation

Use the `#[validate]` attribute to validate JSON request bodies directly in handler functions:

```rust
use larastvel_core::validate;
use larastvel_core::validation::rules::{required, email, min};
use axum::{Json, extract::Json as JsonExtractor};
use serde_json::{json, Value};

#[validate(vec![
    ("email", vec![required(), email()]),
    ("name", vec![required(), min(2)]),
])]
async fn store(Json(body): JsonExtractor<Value>) -> impl IntoResponse {
    Json(json!({"ok": true}))
}
```

The macro:
- Finds the `Json<Value>` parameter in the handler signature
- Converts the body to a `HashMap<String, Value>`
- Runs the validator; returns `422 Unprocessable Entity` with error details on failure
- Passes through to the original handler body on success

Can be combined with `#[route]`:

```rust
#[route]
impl UserController {
    #[post("/users")]
    #[validate(vec![
        ("email", vec![required(), email()]),
    ])]
    async fn create(Json(body): JsonExtractor<Value>) -> impl IntoResponse {
        Json(json!({"created": true}))
    }
}
```

## Query String Validation

Use the `#[validated_query]` attribute to validate query-string parameters:

```rust
use larastvel_core::validated_query;
use larastvel_core::validation::rules::{required, min};
use axum::extract::Query;
use std::collections::HashMap;

#[validated_query(vec![
    ("page", vec![required()]),
    ("per_page", vec![min(1)]),
])]
async fn list(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    Json(json!({"page": params.get("page")}))
}
```

Works inside `#[route]` blocks and composes with `#[can]` and `#[validate]`:

```rust
#[route]
impl SearchController {
    #[get("/search")]
    #[can("admin")]
    #[validated_query(vec![("q", vec![required()])])]
    #[validate(vec![("email", vec![required(), email()])])]
    async fn search(
        Query(params): Query<HashMap<String, String>>,
        Json(body): Json<Value>,
    ) -> impl IntoResponse {
        Json(json!({ "query": params.get("q") }))
    }
}
```

## Extractor-Based Validation

Use `ValidatedJson` and `ValidatedQuery` to auto-validate incoming data:

```rust
use larastvel_core::validation::{ValidatedJson, ValidatedQuery};

async fn create_user(ValidatedJson(data): ValidatedJson<CreateUserRequest>) -> Json<User> {
    // data is already deserialized
}

async fn search(ValidatedQuery(query): ValidatedQuery<SearchParams>) -> Json<Vec<Result>> {
    // query params are validated
}
```

## Custom Error Messages

```rust
let validator = Validator::new(&data, vec![
    ("email", vec![rules::required()]),
]).with_messages(HashMap::from([
    ("email".to_string(), "Please provide your email address.".to_string()),
]));

if validator.fails() {
    // handle errors
}
```

## ValidationErrors API

| Method | Description |
|--------|-------------|
| `has(field)` | Check if field has errors |
| `first(field)` | Get first error for field |
| `all()` | Get all errors |
| `is_empty()` | Check if no errors |
| `to_json()` | Serialize to JSON |
