use larastvel_core::{axum, routing::Registrar};

pub fn api(router: &Registrar) {
    router.group("/api", |r| {
        r.get("/health", || async {
            axum::response::Json(serde_json::json!({
                "status": "ok",
                "framework": "Larastvel",
                "version": "0.2.0",
            }))
        });
    });
}
