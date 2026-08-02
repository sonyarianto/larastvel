# Testing

Larastvel provides testing utilities inspired by Laravel's testing helpers.

## TestClient

```rust
use larastvel_testing::{TestClient, TestResponse};

#[tokio::test]
async fn test_homepage() {
    let app = build_app();
    let client = TestClient::new(app);

    let response = client.get("/").await;
    assert_eq!(response.status(), 200);
    assert!(response.content().contains("Welcome"));
}
```

## TestResponse Methods

```rust
let resp = client.post_json("/login", &json!({"email": "test@test.com", "password": "secret"})).await;

resp.status();         // HTTP status code
resp.headers();        // response headers
resp.content();        // response body as &str
resp.json();           // parse as JSON (serde_json::Value)
```

## RefreshDatabase

```rust
use larastvel_testing::RefreshDatabase;

struct UserTest;

#[async_trait::async_trait]
impl RefreshDatabase for UserTest {
    type Migrator = Migrator; // your generated sea-orm-migration Migrator

    async fn refresh_database(&self, db: &DatabaseManager) {
        db.migrate_fresh::<Self::Migrator>()
            .await
            .expect("Failed to refresh database");
    }
}

#[tokio::test]
async fn test_create_user() {
    let test = UserTest;
    test.refresh_database(&DatabaseManager::new(&config)).await;

    let client = TestClient::new(build_app());
    let resp = client.post_json("/users", &json!({"name": "John", "email": "john@test.com"})).await;

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
