use std::sync::Arc;

use sea_orm::{ConnectOptions, Database, DbConn};
use sea_orm_migration::MigratorTrait;
use tokio::sync::RwLock;
use tracing::info;

use crate::config::Config;

#[async_trait::async_trait]
pub trait Seeder {
    fn name() -> &'static str;
    async fn run(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct DatabaseSeeder;

impl DatabaseSeeder {
    pub async fn run_all(_conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
        info!("Running database seeders");
        Ok(())
    }

    pub async fn run_seeder<S: Seeder>(conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
        info!("Running seeder: {}", S::name());
        S::run(conn).await
    }
}

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

    pub async fn migrate<M: MigratorTrait>(&self) -> Result<(), sea_orm::DbErr> {
        let conn = self.connect().await?;
        info!("Running database migrations");
        M::up(&conn, None).await
    }

    pub async fn migrate_fresh<M: MigratorTrait>(&self) -> Result<(), sea_orm::DbErr> {
        let conn = self.connect().await?;
        info!("Running fresh database migrations (dropping all tables)");
        M::fresh(&conn).await
    }

    pub async fn migrate_rollback<M: MigratorTrait>(&self, steps: Option<u32>) -> Result<(), sea_orm::DbErr> {
        let conn = self.connect().await?;
        info!("Rolling back database migrations");
        M::down(&conn, steps).await
    }

    pub async fn migrate_status<M: MigratorTrait>(&self) -> Result<(), sea_orm::DbErr> {
        let conn = self.connect().await?;
        M::status(&conn).await
    }

    pub async fn seed<S: Seeder>(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connect().await?;
        DatabaseSeeder::run_seeder::<S>(&conn).await
    }

    pub async fn seed_all(&self) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.connect().await?;
        DatabaseSeeder::run_all(&conn).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSeeder;

    #[async_trait::async_trait]
    impl Seeder for TestSeeder {
        fn name() -> &'static str {
            "test_seeder"
        }

        async fn run(_conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    #[test]
    fn test_seeder_trait_compiles() {
        assert_eq!(TestSeeder::name(), "test_seeder");
    }

    #[test]
    fn test_database_seeder_static_methods_exist() {
        // Verify associated functions compile and are callable
        let _ = DatabaseSeeder::run_all;
        let _ = DatabaseSeeder::run_seeder::<TestSeeder>;
    }
}
