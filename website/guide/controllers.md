# Controllers

Controllers group related request-handling logic into a single struct.

## Basic Controllers

Use the `#[controller]` attribute macro:

```rust
use larastvel_core::controller;

#[controller]
impl UserController {
    async fn index() -> Json<Vec<User>> {
        // GET /users
    }

    async fn show(Path(id): Path<i32>) -> Json<User> {
        // GET /users/:id
    }
}
```

## Resource Controllers

The `ResourceController` trait provides default implementations for RESTful actions:

```rust
use larastvel_core::routing::ResourceController;
use larastvel_core::Resource;

#[derive(Resource)]
struct PostResource;
```

Generates routes for: `index`, `create`, `store`, `show`, `edit`, `update`, `destroy`.

Override specific methods:

```rust
#[async_trait]
impl ResourceController for PostResource {
    const RESOURCE_NAME: &'static str = "posts";

    async fn index() -> Response {
        // Custom index logic
    }
}
```

## Manual Resource Registration

For full control, implement `ResourceController` manually:

```rust
impl PostController {
    fn register_routes(registrar: &Registrar) {
        registrar.get("/posts", Self::index);
        registrar.get("/posts/{id}", Self::show);
        registrar.post("/posts", Self::store);
    }
}
```

## Route Registration

Connect controllers to routes in `src/routes/web.rs`:

```rust
pub fn web(router: &Registrar) {
    PostResource::register_routes(router);
}
```
