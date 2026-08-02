# Routing

Larastvel's router is built on Axum (0.8) and wrapped in a `Registrar` that provides a Laravel-like API. Route parameters use Axum's `{id}` syntax.

## Basic Routes

```rust
router.get("/", home_page);
router.post("/login", login_handler);
router.put("/user/{id}", update_user);
router.delete("/user/{id}", delete_user);
```

`get`, `post`, `put`, `patch`, `delete`, and `ws` take a URI and a handler and register the route on the underlying Axum router.

## Route Model Binding

The `ModelPath<E>` extractor resolves a `{id}` route parameter into a model
instance by primary key — Laravel's implicit route model binding. When the
model (or a parseable id) is missing, the handler returns `404` automatically:

```rust
use larastvel_core::routing::ModelPath;
use crate::models::user;

router.get("/users/{id}", |user: ModelPath<user::Entity>| async move {
    Json(json!({ "user": user.0 }))
});
```

The database connection is taken from an `Extension<DatabaseConnection>` layer
when present, otherwise it falls back to the global connection registered via
`set_global_database()`.

### Route Key Binding

Like Laravel 13.21's `#[RouteKey]` attribute, a model can be bound by a
non-primary-key column (e.g. a slug). Set `route_key` on the `#[table]`
macro to declare the column — the extractor then resolves `{post}` by that
column instead of the primary key, still returning `404` when no row matches:

```rust
use larastvel_core::routing::ModelPath;

#[table("posts", route_key = "slug")]
struct Post {
    #[sea_orm(primary_key)]
    id: i32,
    slug: String,
    title: String,
}

router.get("/posts/{post}", |post: ModelPath<post::Entity>| async move {
    Json(json!({ "post": post.0 }))
});
```

## Route Groups

```rust
router.group("/admin", |r| {
    r.get("/dashboard", admin_dashboard);
    r.get("/users", admin_users);
});
```

Routes registered inside the closure are prefixed with the group path.

## Route Listing

`Registrar::list_routes()` returns the registered routes with their methods, URIs, and middleware:

```rust
for route in router.list_routes() {
    println!("{} {}", route.method, route.uri); // e.g. "GET /users"
}
```

## Middleware

### Registering Middleware Aliases

Middleware aliases are registered with a name and a function that transforms the `MethodRouter` (usually applying an Axum layer):

```rust
router.middleware("auth", |r| r.layer(auth_layer));
router.middleware("throttle", |r| r.layer(rate_limit_layer));
```

### Applying Middleware

Apply previously-registered aliases to all routes registered afterwards:

```rust
router.with_middleware(vec!["auth", "throttle"]);

router.get("/dashboard", dashboard_handler);
```

Groups restore the previous middleware list when they exit:

```rust
router.group("/admin", |r| {
    r.with_middleware(vec!["auth"]);
    r.get("/", admin_index);
});
```

### Per-Route Middleware

Inside a `#[route]` impl block, use the `#[middleware]` attribute to attach middleware aliases to a single handler:

```rust
#[route]
impl AdminController {
    #[get("/admin")]
    #[middleware("auth")]
    async fn admin_index() -> impl IntoResponse {
        Html("<h1>Admin</h1>")
    }
}
```

## Authorization

Use `#[can("ability")]` to protect handler functions:

```rust
#[can("admin")]
async fn admin_dashboard(Extension(state): Extension<AppState>) -> impl IntoResponse {
    Html("<h1>Admin Dashboard</h1>")
}

// Usage in routes:
router.get("/admin", admin_dashboard);
```

The macro injects `AuthenticatedUser` and `Extension<Gate>` as the first extractor parameters and checks the ability before the handler body runs. Returns `403 Forbidden` if denied.

> **Note:** The `Gate` must be available in the Axum request extensions via `.layer(Extension(gate))`.

### With Route Macros

```rust
#[route]
impl AdminController {
    #[get("/admin")]
    #[can("admin")]
    async fn dashboard() -> impl IntoResponse {
        Html("<h1>Admin</h1>")
    }
}
```

## Route Attribute Macro

The `#[route]` macro lets you define routes directly on controller methods using `#[get]`, `#[post]`, `#[put]`, `#[patch]`, `#[delete]`, and `#[ws]` attributes:

```rust
#[route]
impl UserController {
    #[get("/users")]
    async fn index() -> impl IntoResponse {
        Json(json!({"users": []}))
    }

    #[post("/users")]
    async fn store() -> impl IntoResponse {
        StatusCode::CREATED
    }

    #[get("/users/{id}")]
    async fn show(Path(id): Path<String>) -> impl IntoResponse {
        Json(json!({"user": {"id": id}}))
    }

    #[put("/users/{id}")]
    async fn update(Path(id): Path<String>) -> impl IntoResponse {
        Json(json!({"updated": true}))
    }

    #[delete("/users/{id}")]
    async fn destroy(Path(id): Path<String>) -> impl IntoResponse {
        StatusCode::NO_CONTENT
    }
}
```

The macro generates a `register_routes(&Registrar)` method on the struct. Call it in your route files:

```rust
// routes/api.rs
use larastvel_core::routing::Registrar;

pub fn api(router: &Registrar) {
    UserController::register_routes(router);
}
```

Methods without a route attribute are left as-is (not registered). Each method is an Axum handler and can use any Axum extractor.

## Controllers

A controller is an `impl` block that registers its own routes:

```rust
use larastvel_core::routing::Registrar;

struct UserController;

impl UserController {
    fn register_routes(router: &Registrar) {
        router.get("/users", Self::index);
        router.get("/users/{id}", Self::show);
    }

    async fn index() -> Json<Vec<User>> {
        // GET /users
    }

    async fn show(Path(id): Path<i32>) -> Json<User> {
        // GET /users/{id}
    }
}
```

The `#[controller]` attribute is a marker that also generates an empty `register_routes(&Registrar)`; the routes themselves are registered manually as above (or with `#[route]`).

## Resources

`#[derive(Resource)]` implements the `ResourceController` trait (with the resource name derived from the struct name) and generates a `register_routes(&Registrar)` that registers all seven resource routes:

```rust
use larastvel_core::Resource;

#[derive(Resource)]
struct UserResource;

// Register all resource routes
UserResource::register_routes(router);
```

Generates: `GET /userresource`, `GET /userresource/create`, `POST /userresource`, `GET /userresource/{id}`, `GET /userresource/{id}/edit`, `PUT /userresource/{id}`, `DELETE /userresource/{id}`.

Override the default handlers by implementing `ResourceController`:

```rust
#[async_trait::async_trait]
impl ResourceController for UserResource {
    const RESOURCE_NAME: &'static str = "users";

    async fn index() -> Response {
        Json(json!({"users": []})).into_response()
    }
}
```

## WebSocket

```rust
router.ws("/ws", ws_handler);
```

See the broadcasting docs for a full WebSocket example with NativeBroadcaster.

## Signed URLs

Sign a path (or full URL) with a secret key so tampering and expiry are detectable — Laravel's `URL::signedRoute()`:

```rust
use larastvel_core::{signed_route, has_valid_signature};

// Build a signed URL. ttl is optional; when given, an `expires` param is
// added and enforced.
let url = signed_route("/verify-email", &[("user", "42")], Some(Duration::from_secs(3600)), APP_KEY)?;

// Verify an incoming request target (path + query string)
if has_valid_signature(&request_uri, APP_KEY) {
    // signature valid and not expired
}
```

Properties:

- HMAC-SHA256 per RFC 2104 (constant-time comparison, so the signature check does not leak timing information).
- Query parameters are canonicalized (sorted) before signing, so the order in the URL does not matter.
- With a `ttl`, the `expires` parameter is included in the signature, so an expired URL cannot be "un-expired" by editing the query string.
- Missing or malformed signatures, wrong keys, and expired links all return `false` from `has_valid_signature`.

## Route Listing

```bash
cargo run -p larastvel-cli -- route:list
```

Lists all registered routes with methods, URIs, and middleware.
