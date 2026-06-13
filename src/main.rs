mod routes;

use larastvel_core::{Application, DatabaseManager, logging};

#[tokio::main]
async fn main() {
    let app = Application::new(None);
    logging::init(&app.config());

    let db = DatabaseManager::new(&app.config());
    match db.connect().await {
        Ok(_) => tracing::info!("Database connected successfully"),
        Err(e) => tracing::warn!("Database connection failed: {} (app will still run)", e),
    }
    let app = app.with_database(db);

    let router = app.router();
    routes::web::web(&router);
    routes::api::api(&router);

    app.run().await;
}
