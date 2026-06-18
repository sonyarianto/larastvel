# Testing

Larastvel provides testing utilities inspired by Laravel's testing helpers.

## TestClient

```rust
use larastvel_core::testing::{TestClient, TestResponse};

#[tokio::test]
async fn test_homepage() {
    let app = build_app();
    let client = TestClient::new(app);

    let response = client.get("/").await;
    assert_eq!(response.status(), 200);
    assert!(response.body().contains("Welcome"));
}
```

## TestResponse Methods

```rust
let resp = client.post("/login")
    .json(&json!({"email": "test@test.com", "password": "secret"}))
    .await;

resp.status();         // HTTP status code
resp.headers();        // response headers
resp.body();           // response body as bytes
resp.json::<Value>();  // parse as JSON
```

## RefreshDatabase

```rust
use larastvel_core::testing::RefreshDatabase;

struct UserTest;

#[async_trait]
impl RefreshDatabase for UserTest {
    async fn refresh_database(&self) {
        // run migrations
    }
}

#[tokio::test]
async fn test_create_user() {
    let test = UserTest;
    test.refresh_database().await;

    let client = TestClient::new(build_app());
    let resp = client.post("/users")
        .json(&json!({"name": "John", "email": "john@test.com"}))
        .await;

    assert_eq!(resp.status(), 201);
}
```

## Testing Events

```rust
EventService::fake();

// perform action that dispatches events
EventService::dispatch(OrderShipped { order_id: "1".into() }).await;

assert!(EventService::assert_dispatched::<OrderShipped>());
EventService::reset();
```

## Running Tests

```bash
cargo test --workspace
```
