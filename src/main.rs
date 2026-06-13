mod database;
mod models;
mod routes;

use database::migrator::Migrator;
use larastvel_core::{logging, Application, DatabaseManager};

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

    let router = app.router();
    routes::web::web(&router);
    routes::api::api(&router);

    app.run().await;
}
