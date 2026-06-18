# Arr (Array Helpers)

Larastvel's `Arr` provides a set of static array utility methods inspired by Laravel's `Illuminate\Support\Arr`.

## List Operations

```rust
use larastvel_core::Arr;

Arr::wrap("hello");                    // ["hello"]

Arr::first(&[1, 2, 3]);                // Some(1)
Arr::last(&[1, 2, 3]);                 // Some(3)

Arr::join(&[1, 2, 3], ", ");           // "1, 2, 3"
Arr::join_last(&["a", "b", "c"], ", ", " and "); // "a, b and c"

let data = vec![vec![1, 2], vec![3], vec![4, 5]];
Arr::collapse(data);                   // [1, 2, 3, 4, 5]

Arr::cross_join(&[vec![1, 2], vec!['a', 'b']]);
// [[1, 'a'], [1, 'b'], [2, 'a'], [2, 'b']]
```

## Randomization

```rust
Arr::random(&[10, 20, 30]);            // Some(20) — random element
Arr::random_count(&[1, 2, 3, 4, 5], 3); // e.g. [4, 1, 3] — 3 random items

let mut items = [1, 2, 3, 4, 5];
Arr::shuffle(&mut items);              // items is now shuffled
```

## Nested Array Operations

```rust
use serde_json::json;

let data = json!({
    "user": {
        "name": "John",
        "address": { "city": "New York" }
    }
});

Arr::get(&data, "user.name");           // Some("John")
Arr::get(&data, "user.address.city");   // Some("New York")
Arr::has(&data, "user.name");           // true
Arr::has_any(&data, &["name", "foo"]);  // true

let mut data = json!({"user": {"name": "John"}});
Arr::set(&mut data, "user.name", json!("Jane"));

let mut data = json!({"items": [1, 2, 3]});
Arr::forget(&mut data, "items.1");
// data is now {"items": [1, 3]}

let mut data = json!({"user": {"name": "John", "age": 30}});
let name = Arr::pull(&mut data, "user.name");
// name is Some("John"), data is now {"user": {"age": 30}}
```

## Dot Notation

```rust
let data = json!({
    "user": {
        "name": "John",
        "address": { "city": "New York" }
    },
    "active": true
});

// Flatten to dot notation
let dotted = Arr::dot(&data, "");
// { "user.name": "John", "user.address.city": "New York", "active": true }

// Expand back to nested
let undotted = Arr::undot(&dotted);
// { "user": { "name": "John", "address": { "city": "New York" } }, "active": true }
```

## Map Operations

```rust
use serde_json::json;
use std::collections::HashMap;

let map = json!({"name": "John", "age": 30, "email": "john@test.com"});
let map = map.as_object().unwrap();

Arr::only(map, &["name", "email"]);
// { "name": "John", "email": "john@test.com" }

Arr::except(map, &["age"]);
// { "name": "John", "email": "john@test.com" }

let mut map = Map::new();
Arr::add(&mut map, "key", json!("val"));
// { "key": "val" } — only adds if key doesn't exist

let mut map = Map::new();
map.insert("name".into(), json!("John"));
map.insert("age".into(), json!(30));
let prefixed = Arr::prepend_keys_with(map, "user_");
// { "user_name": "John", "user_age": 30 }

let mut map = HashMap::new();
map.insert("a", 1);
map.insert("b", 2);
let (keys, values) = Arr::divide(map);
// keys = ["a", "b"], values = [1, 2]
```

## Sorting

```rust
// Recursively sort arrays and object keys
let mut data = json!({"b": [3, 1, 2], "a": [6, 5]});
Arr::sort_recursive(&mut data);
// {"a": [5, 6], "b": [1, 2, 3]}
```

## Type Checks

```rust
Arr::is_assoc(&json!({"key": "val"}));         // true
Arr::is_assoc(&json!([1, 2, 3]));              // false
Arr::is_list(&json!([1, 2, 3]));               // true
Arr::is_list(&json!({"key": "val"}));           // false
```

## Flatten

```rust
let nested = json!([1, [2, 3], [4, [5, 6]]]);
Arr::flatten(&nested);
// [1, 2, 3, 4, 5, 6]
```

## Available Methods

| Method | Description |
|--------|-------------|
| `wrap(value)` | Wrap a single value in a Vec |
| `first(items)` | Get the first element |
| `last(items)` | Get the last element |
| `random(items)` | Get a random element |
| `random_count(items, count)` | Get N random elements |
| `join(items, glue)` | Join elements as a string |
| `join_last(items, glue, last_glue)` | Join with Oxford comma style |
| `collapse(arrays)` | Collapse an array of arrays one level |
| `cross_join(...)` | Cross join multiple arrays |
| `shuffle(items)` | Shuffle the array in place |
| `sort_recursive(value)` | Recursively sort arrays and object keys |
| `flatten(value)` | Flatten nested arrays |
| `divide(map)` | Split a HashMap into keys and values |
| `only(map, keys)` | Keep only specified keys |
| `except(map, keys)` | Remove specified keys |
| `add(map, key, value)` | Add key if it doesn't exist |
| `prepend_keys_with(map, prefix)` | Prefix all keys |
| `is_assoc(value)` | Check if Value is an object |
| `is_list(value)` | Check if Value is an array |
| `get(value, key)` | Get nested value using dot notation |
| `set(value, key, value)` | Set nested value using dot notation |
| `has(value, key)` | Check if key exists (dot notation) |
| `has_any(value, keys)` | Check if any key exists |
| `forget(value, key)` | Remove key (dot notation) |
| `pull(value, key)` | Get and remove key (dot notation) |
| `dot(value, prepend)` | Flatten nested array to dot notation |
| `undot(map)` | Expand dot notation to nested array |
