use colored::*;

pub async fn start_server(host: &str, port: u16) {
    let addr = format!("{}:{}", host, port);
    println!("  Server running on http://{}", addr.green());

    let app = larastvel_core::axum::Router::new().route(
        "/health",
        larastvel_core::axum::routing::get(|| async {
            larastvel_core::axum::response::Json(serde_json::json!({
                "status": "ok",
                "framework": "Larastvel",
                "version": env!("CARGO_PKG_VERSION"),
            }))
        }),
    );

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    larastvel_core::axum::serve(listener, app).await.unwrap();
}
