# HTTP Client

Larastvel provides a fluent, minimal-wrapper around `reqwest` for making HTTP requests to external services.

## Making Requests

```rust
use larastvel_core::Http;

let response = Http::new()
    .get("https://api.example.com/users")
    .await?;

let users: Vec<User> = response.json().await?;
```

### Request Methods

```rust
Http::new().get(url).await?
Http::new().post(url).await?
Http::new().put(url).await?
Http::new().patch(url).await?
Http::new().delete(url).await?
Http::new().head(url).await?
```

## Headers & Authentication

```rust
// Bearer token
let response = Http::with_token("your-token")
    .get("https://api.github.com/user")
    .await?;

// Basic auth
let response = Http::with_basic_auth("user", "pass")
    .get("https://api.example.com/protected")
    .await?;

// Custom headers
let response = Http::new()
    .with_header("X-Custom", "value")
    .with_headers([("Accept-Language", "en")].into())
    .get("https://api.example.com/data")
    .await?;

// Accept JSON shorthand
let response = Http::accept_json()
    .get("https://api.example.com/data.json")
    .await?;
```

## Request Body

```rust
use serde_json::json;

// JSON body
let response = Http::new()
    .json(&json!({"name": "John", "email": "john@example.com"}))
    .post("https://api.example.com/users")
    .await?;

// Form-encoded body
let response = Http::new()
    .as_form(vec![("username", "john"), ("password", "secret")])
    .post("https://api.example.com/login")
    .await?;

// Raw body
let response = Http::new()
    .body("raw string body")
    .post("https://api.example.com/echo")
    .await?;
```

## Query Parameters

```rust
let response = Http::new()
    .with_query(vec![("page", "1"), ("per_page", "20")])
    .get("https://api.example.com/users")
    .await?;
```

## Base URL

```rust
let client = Http::base_url("https://api.example.com/v1");

let users = client.get("/users").await?;    // https://api.example.com/v1/users
let posts = client.get("/posts").await?;    // https://api.example.com/v1/posts
```

## Timeouts & Retries

```rust
use std::time::Duration;

// Set a timeout
let response = Http::timeout(Duration::from_secs(10))
    .get("https://api.example.com/slow")
    .await?;

// Retry with delay
let response = Http::new()
    .retry(3, Duration::from_millis(500))
    .get("https://api.example.com/unstable")
    .await?;
```

## Responses

```rust
let response = Http::new()
    .get("https://api.example.com/resource")
    .await?;

response.status()                           // reqwest::StatusCode
response.ok()                               // bool (2xx)
response.client_error()                     // bool (4xx)
response.server_error()                     // bool (5xx)
response.status_code()                      // u16

let text: String = response.text().await?;
let json: Value = response.json().await?;
let bytes: Vec<u8> = response.body().await?;
let header = response.header("content-type");
```

## Error Handling

```rust
use larastvel_core::PendingRequest;

match Http::new().get("https://api.example.com/data").await {
    Ok(response) => {
        let data: MyData = response.json().await?;
        // handle success
    }
    Err(e) => {
        eprintln!("Request failed: {}", e);
    }
}
```
