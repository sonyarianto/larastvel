use axum::Router;
use axum::routing::get;
use axum::response::Json;
use serde_json::json;
use larastvel_testing::TestClient;

#[tokio::test]
async fn test_get_request() {
    let app = Router::new()
        .route("/hello", get(|| async { Json(json!({"message": "Hello, world!"})) }));

    let client = TestClient::new(app);

    let resp = client.get("/hello").await;

    resp.assert_ok()
        .assert_json(json!({"message": "Hello, world!"}))
        .assert_json_path("message", json!("Hello, world!"))
        .assert_header("content-type", "application/json");
}

#[tokio::test]
async fn test_not_found() {
    let app = Router::new()
        .route("/exists", get(|| async { "ok" }));

    let client = TestClient::new(app);

    let resp = client.get("/nonexistent").await;
    resp.assert_not_found();
}

#[tokio::test]
async fn test_see_text() {
    let app = Router::new()
        .route("/page", get(|| async { "<html><body><h1>Welcome</h1><p>to Larastvel</p></body></html>" }));

    let client = TestClient::new(app);

    let resp = client.get("/page").await;
    resp.assert_ok()
        .assert_see("Welcome")
        .assert_see("Larastvel")
        .assert_see_in_order(&["<h1>", "Welcome", "</h1>"])
        .assert_dont_see("Goodbye")
        .assert_see_text("Welcome to Larastvel");
}

#[tokio::test]
async fn test_post_json() {
    use serde_json::Value;
    use axum::routing::post;

    let app = Router::new()
        .route("/echo", post(|body: axum::Json<Value>| async move { Json(body.0) }));

    let client = TestClient::new(app);

    let resp = client.post_json("/echo", &json!({"key": "value"})).await;
    resp.assert_ok()
        .assert_json(json!({"key": "value"}));
}

#[tokio::test]
async fn test_headers_and_auth() {
    let app = Router::new()
        .route("/protected", get(|headers: axum::http::HeaderMap| async move {
            let auth = headers.get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("none");
            Json(json!({"auth": auth}))
        }));

    let client = TestClient::new(app)
        .with_bearer_token("my-token");

    let resp = client.get("/protected").await;
    resp.assert_ok()
        .assert_json_path("auth", json!("Bearer my-token"));
}

#[tokio::test]
async fn test_json_missing_and_structure() {
    let app = Router::new()
        .route("/data", get(|| async { Json(json!({"name": "test", "values": [1, 2, 3]})) }));

    let client = TestClient::new(app);

    let resp = client.get("/data").await;
    resp.assert_ok()
        .assert_json_structure(&["name", "values"])
        .assert_json_missing("nonexistent")
        .assert_json_count("values", 3);
}
