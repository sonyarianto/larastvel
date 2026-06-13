mod database;
mod models;
mod routes;

use std::sync::Arc;

use database::migrator::Migrator;
use larastvel_core::{logging, Application, DatabaseManager, RouteServiceProvider};

#[tokio::main]
async fn main() {
    let app = Application::new(None);
    logging::init(&app.config());

    let db = DatabaseManager::new(&app.config());
    match db.connect().await {
        Ok(conn) => {
            tracing::info!("Database connected successfully");
            let _ = larastvel_core::models::set_global_database(conn);
        }
        Err(e) => tracing::warn!("Database connection failed: {} (app will still run)", e),
    }

    if let Err(e) = db.migrate::<Migrator>().await {
        tracing::warn!("Migration failed: {} (app will still run)", e);
    }

    let app = app.with_database(db);

    // Register service providers
    app.register_provider(Arc::new(
        RouteServiceProvider::new()
            .web(|r| routes::web::web(r))
            .api(|r| routes::api::api(r)),
    ));

    app.run().await;
}
