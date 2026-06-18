use axum::{extract::Request, middleware::Next, response::Response};

pub mod presets;

pub use presets::{auth, cors, guest, logger, throttle, verified};

pub async fn cors_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("Access-Control-Allow-Origin", "*".parse().unwrap());
    response.headers_mut().insert(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, DELETE, PATCH".parse().unwrap(),
    );
    response.headers_mut().insert(
        "Access-Control-Allow-Headers",
        "Content-Type, Authorization".parse().unwrap(),
    );
    response
}

pub async fn request_logger(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    tracing::info!("→ {} {}", method, uri);
    let response = next.run(request).await;
    tracing::info!("← {} {} - {}", method, uri, response.status());
    response
}
