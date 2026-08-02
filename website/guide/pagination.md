# Pagination

Larastvel provides a paginator for paginating query results. Like Laravel 13, the default page size is 25 items per page.

## Basic Usage

```rust
use larastvel_core::pagination::{paginate, PaginationParams, Paginator};

// From request query params
let params: PaginationParams = PaginationParams {
    page: Some(1),
    per_page: Some(25),
};

// Paginate a vector of items
let items = vec!["item1", "item2", /* ... */];
let paginator = paginate(items, params.page.unwrap_or(1), params.per_page.unwrap_or(25));

// Get paginated results
let paginated = paginator.to_json();
```

## Paginator API

```rust
let paginator = Paginator::new(
    items,           // Vec<T>
    total,           // usize
    page,            // usize
    per_page,        // usize
);

paginator.items();       // current page items
paginator.total();       // total items
paginator.last_page();   // last page number
paginator.has_pages();   // more than one page?
paginator.has_more();    // has next page?
paginator.to_json();     // serialize to JSON
```

## Response Format

```json
{
  "data": [...],
  "total": 100,
  "per_page": 25,
  "current_page": 1,
  "last_page": 7,
  "has_more": true
}
```
