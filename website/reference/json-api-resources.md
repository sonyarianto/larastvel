# JSON:API Resources

Larastvel ships first-party [JSON:API](https://jsonapi.org/) support, mirroring Laravel 13's `JsonApiResource`. Resources produce spec-compliant responses with `id`, `type`, `attributes`, `relationships`, `links`, and `meta`, automatic sparse fieldset filtering, compound documents via `?include=`, and the `application/vnd.api+json` content type.

## Defining a Resource

Implement the `JsonApiResource` trait. Only `attributes` is required:

```rust
use larastvel_core::models::jsonapi::{JsonApiQuery, JsonApiResource, when_included};
use serde_json::{json, Value};

struct PostResource;

impl JsonApiResource<crate::models::post::Model> for PostResource {
    fn attributes(model: &crate::models::post::Model) -> Value {
        json!({
            "title": model.title,
            "body": model.body,
        })
    }
}
```

### Optional Methods

| Method | Default | Description |
|--------|---------|-------------|
| `id(model)` | serialized `id` field, stringified | The resource `id` (spec requires strings) |
| `type_()` | model type name lowercased (`User` → `user`) | The resource `type`; override for pluralised types |
| `relationships(model, query)` | `{}` | Relationship linkage, use with `when_included` |
| `links(model)` | `{}` | e.g. a `self` link |
| `meta(model)` | `{}` | Resource-level metadata |

```rust
impl JsonApiResource<crate::models::post::Model> for PostResource {
    fn attributes(model: &crate::models::post::Model) -> Value {
        json!({ "title": model.title, "body": model.body })
    }

    fn id(model: &crate::models::post::Model) -> String {
        model.uuid.clone()
    }

    fn type_() -> String {
        "articles".into()
    }

    fn relationships(model: &crate::models::post::Model, query: &JsonApiQuery) -> Value {
        json!({
            "author": when_included(query, "author", json!({
                "data": { "type": "users", "id": model.author_id.to_string() },
            })),
        })
    }

    fn links(model: &crate::models::post::Model) -> Value {
        json!({ "self": format!("/api/posts/{}", model.id) })
    }
}
```

`when_included(query, relationship, value)` emits the value only when the client requested the relationship via `?include=`. Unrequested entries are dropped from the response (never `null`). `when_not_included` is the inverse.

## Sparse Fieldsets

Clients can request only the attributes they need with the `fields[type]` query parameter:

```text
GET /api/posts?fields[posts]=title,created_at&fields[users]=name
```

Filtering is automatic — parse the request query string and pass it to the document:

```rust
let query = JsonApiQuery::parse(uri.query().unwrap_or(""));

let doc = PostResource::make(post)
    .with_query(&query)
    .to_array();
```

Per the JSON:API spec, sparse fieldsets only ever filter `attributes` — `relationships` linkage is always present.

## Compound Documents

Register related resources with `include()` / `include_collection()` and they appear in the top-level `included` array when requested. Included resources are deduplicated by `type` + `id` and honour sparse fieldsets for their own type. Nested paths work too: requesting `?include=author.posts` also includes `author`.

```rust
let query = JsonApiQuery::parse(uri.query().unwrap_or(""));

let doc = PostResource::make(post)
    .with_query(&query)
    .include("author", UserResource::make(author))
    .include_collection("comments", CommentResource::collection(comments))
    .to_array();
```

```json
{
  "data": {
    "id": "1",
    "type": "articles",
    "attributes": { "title": "Hello World" },
    "relationships": {
      "author": { "data": { "type": "users", "id": "7" } }
    }
  },
  "included": [
    { "type": "users", "id": "7", "attributes": { "name": "Tim" } },
    { "type": "comments", "id": "1", "attributes": { "body": "Nice!" } }
  ]
}
```

### Ignoring the Query String

Webhooks and internal endpoints may want to ignore client field/include requests — the equivalent of Laravel's `ignoreFieldsAndIncludesInQueryString()`:

```rust
PostResource::make(post).ignore_query_string()
```

## Collections

```rust
let doc = PostResource::collection(posts)
    .with_query(&query)
    .include("author", UserResource::make(author))
    .with_links(json!({ "self": "/api/posts" }))
    .with_meta(json!({ "total": 42 }))
    .to_array();
```

Collections emit `data` (array), an automatic `meta.count`, any `links` / extra `meta` set via `with_links()` / `with_meta()`, and the aggregated `included` array. On single resources, additional top-level keys can be merged with `additional()`.

## Responses

Both single resources and collections implement `IntoResponse` with the `application/vnd.api+json` content type, so they can be returned directly from controllers:

```rust
#[get("/posts/{id}")]
async fn show(Path(id): Path<u64>) -> impl IntoResponse {
    // ... load post ...
    PostResource::make(post).with_query(&query)
}
```

## The `#[json_api_resource]` Macro

The attribute macro generates the trait implementation from inherent methods on your resource:

```rust
use larastvel_core::json_api_resource;

#[json_api_resource(crate::models::post::Model)]
impl PostResource {
    fn attributes(model: &crate::models::post::Model) -> serde_json::Value {
        serde_json::json!({ "title": model.title })
    }

    fn id(model: &crate::models::post::Model) -> String {
        format!("post-{}", model.id)
    }

    fn type_() -> String {
        "articles".into()
    }

    fn relationships(model: &crate::models::post::Model, query: &JsonApiQuery) -> serde_json::Value {
        serde_json::json!({
            "author": when_included(query, "author", serde_json::json!({
                "data": { "type": "users", "id": model.author_id.to_string() },
            })),
        })
    }
}
```

Any subset of `attributes`, `id`, `type_`, `relationships`, `links`, and `meta` may be defined; the rest use trait defaults.

## Related

- [API Resources](/reference/api-resources) — the plain (non-JSON:API) resource layer
- [Database & ORM](/guide/database) — models and serialization
