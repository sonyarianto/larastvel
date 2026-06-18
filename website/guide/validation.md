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
