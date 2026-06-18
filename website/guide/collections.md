# Collections

Larastvel's `Collection<T>` provides a fluent, expressive wrapper around `Vec<T>` for working with arrays of data. It offers a rich API for mapping, filtering, sorting, and transforming data without manual loops.

## Creating Collections

```rust
use larastvel_core::Collection;

// From a Vec
let collection = Collection::new(vec![1, 2, 3, 4, 5]);

// From any iterator
let collection = Collection::collect(0..10);

// Using the collect helper
use larastvel_core::collect_items;
let collection = collect_items(vec!["a", "b", "c"]);
```

## Available Methods

| Method | Description |
|--------|-------------|
| `count()` | Return the number of items |
| `is_empty()` | Check if the collection is empty |
| `first()` | Get the first item |
| `last()` | Get the last item |
| `get(index)` | Get an item by index |
| `map(f)` | Transform each item |
| `filter(f)` | Filter items by a predicate |
| `reduce(f)` | Reduce to a single value |
| `fold(init, f)` | Fold with an initial value |
| `sort()` | Sort items (requires `T: Ord + Clone`) |
| `sort_by(f)` | Sort items with a custom comparator |
| `reverse()` | Reverse item order |
| `take(n)` | Take the first `n` items |
| `skip(n)` | Skip the first `n` items |
| `unique(f)` | Get unique items by a key |
| `chunk(size)` | Split into chunks |
| `each(f)` | Execute a side effect for each item |
| `pluck(f)` | Extract a field from each item |
| `to_json()` | Serialize to JSON |

## Examples

### Mapping and Filtering

```rust
let result = Collection::new(vec![1, 2, 3, 4, 5, 6])
    .filter(|x| x % 2 == 0)
    .cloned()
    .map(|x| x * 10);
// [20, 40, 60]
```

### Sorting and Reversing

```rust
let sorted = Collection::new(vec![3, 1, 4, 1, 5, 9])
    .sort()
    .reverse();
// [9, 5, 4, 3, 1, 1]
```

### Chunking

```rust
let chunks = Collection::new(vec![1, 2, 3, 4, 5]).chunk(2);
// chunks.count() == 3
// chunks.get(0) == Some(&vec![&1, &2])
```

### JSON Serialization

```rust
let json = Collection::new(vec!["a", "b", "c"]).to_json();
// ["a", "b", "c"]
```

### Converting

```rust
let vec: Vec<i32> = Collection::new(vec![1, 2, 3]).into();
let collection: Collection<i32> = vec![1, 2, 3].into();
```
