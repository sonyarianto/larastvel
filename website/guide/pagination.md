# Pagination

Larastvel provides a paginator for paginating query results. Like Laravel 13, the default page size is 25 items per page.

## Basic Usage

```rust
use larastvel_core::pagination::{paginate, PaginationParams, Paginator};

// From request query params (or build manually)
let params = PaginationParams::new(Some(1), Some(25));

// Paginate a vector of items
let items = vec!["item1", "item2", /* ... */];
let paginator = paginate(items, 100, &params);

// Get paginated results
let json = paginator.to_json();
```

## Paginator API

```rust
let paginator = Paginator::new(
    items,           // Vec<T>
    total,           // usize
    current_page,    // usize
    per_page,        // usize
);

paginator.items;          // current page items (pub field)
paginator.total;          // total items (pub field)
paginator.last_page();    // last page number
paginator.has_next_page();// is there a next page?
paginator.has_prev_page();// is there a previous page?
paginator.to_json();      // serialize to JSON
```

## Response Format

`to_json()` returns a `serde_json::Value` with the following shape:

```json
{
  "data": [...],
  "meta": {
    "current_page": 1,
    "last_page": 7,
    "per_page": 25,
    "total": 100,
    "from": 1,
    "to": 25,
    "count": 25
  }
}
```
