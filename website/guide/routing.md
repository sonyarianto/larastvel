# Routing

Larastvel's router is built on Axum and wrapped in a `Registrar` that provides a Laravel-like API.

## Basic Routes

```rust
router.get("/", home_page);
router.post("/login", login_handler);
router.put("/user/:id", update_user);
router.delete("/user/:id", delete_user);
```

## Route Groups

```rust
router.group("/admin", |r| {
    r.get("/dashboard", admin_dashboard);
    r.get("/users", admin_users);
});
```

## Named Routes

```rust
router.get("/user/:id", user_show).name("profile");
// Generate: router.route("profile", &[("id", "42")])
```

## Middleware

### Global Middleware

Middleware can be registered by alias or directly:

```rust
router.middleware("auth", auth_middleware);
router.middleware("throttle:60,1", rate_limiter_middleware);
```

### Per-Route / Per-Group

```rust
router.get("/dashboard", dashboard_handler)
    .middleware("auth")
    .middleware("throttle:60,1");

router.group("/admin", |r| {
    r.middleware("auth");
    r.get("/", admin_index);
});
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
pub fn api(router: &Registrar) {
    UserController::register_routes(router);
}
```

Methods without a route attribute are left as-is (not registered). Each method is an Axum handler and can use any Axum extractor.

## Controllers

Use the `#[controller]` macro:

```rust
#[controller]
impl UserController {
    async fn index() -> Json<Vec<User>> {
        // GET /users
    }

    async fn show(Path(id): Path<i32>) -> Json<User> {
        // GET /users/:id
    }
}

router.get("/users", UserController::index);
router.get("/users/:id", UserController::show);
```

## Resources

```rust
#[derive(Resource)]
#[resource(controller = "UserController")]
struct UserResource;

router.resource("/users", UserResource::routes());
```

Generates: `index`, `create`, `store`, `show`, `edit`, `update`, `destroy`.

## WebSocket

```rust
router.ws("/ws", ws_handler);
```

See the broadcasting docs for a full WebSocket example with NativeBroadcaster.

## Route Listing

```bash
cargo run -p larastvel-cli -- route:list
```

Lists all registered routes with methods, URIs, and middleware.
