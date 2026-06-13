use std::sync::Arc;

use sea_orm::{ConnectOptions, Database, DbConn};
use tokio::sync::RwLock;
use tracing::info;

use crate::config::Config;

#[derive(Clone)]
pub struct DatabaseManager {
    conn: Arc<RwLock<Option<DbConn>>>,
    config: Config,
}

impl DatabaseManager {
    pub fn new(config: &Config) -> Self {
        Self {
            conn: Arc::new(RwLock::new(None)),
            config: config.clone(),
        }
    }

    pub async fn connect(&self) -> Result<DbConn, sea_orm::DbErr> {
        if let Some(conn) = self.conn.read().await.as_ref() {
            return Ok(conn.clone());
        }

        let url = self.build_url();
        info!("Connecting to database: {}", &self.config.database.driver);

        let mut opt = ConnectOptions::new(url);
        opt.max_connections(100)
            .min_connections(5)
            .connect_timeout(std::time::Duration::from_secs(10))
            .idle_timeout(std::time::Duration::from_secs(600))
            .sqlx_logging(self.config.app.debug);

        let conn = Database::connect(opt).await?;
        *self.conn.write().await = Some(conn.clone());
        Ok(conn)
    }

    pub async fn connection(&self) -> Result<DbConn, sea_orm::DbErr> {
        self.connect().await
    }

    pub async fn disconnect(&self) {
        *self.conn.write().await = None;
    }

    fn build_url(&self) -> String {
        let db = &self.config.database;
        match db.driver.as_str() {
            "postgres" | "pgsql" => {
                format!(
                    "postgres://{}:{}@{}:{}/{}",
                    db.username, db.password, db.host, db.port, db.database
                )
            }
            "mysql" | "mariadb" => {
                format!(
                    "mysql://{}:{}@{}:{}/{}",
                    db.username, db.password, db.host, db.port, db.database
                )
            }
            "sqlite" => {
                format!("sqlite://{}?mode=rwc", db.database)
            }
            _ => {
                format!("sqlite://{}?mode=rwc", db.database)
            }
        }
    }

    pub async fn migrate(&self) -> Result<(), Box<dyn std::error::Error>> {
        let _conn = self.connect().await?;
        info!("Running database migrations");
        Ok(())
    }

    pub async fn seed(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Seeding database");
        Ok(())
    }
}
