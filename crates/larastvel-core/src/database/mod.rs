use std::future::Future;
use std::sync::Arc;

use sea_orm::{ConnectOptions, Database, DbConn, TransactionTrait};
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

    /// Run a closure inside a database transaction.
    ///
    /// If the closure returns `Ok`, the transaction is committed. If it
    /// returns `Err`, the transaction is rolled back and the error is
    /// propagated. The closure must be boxed with `Box::pin` because it
    /// borrows the transaction handle:
    ///
    /// ```rust,ignore
    /// let ok = db.transaction(|txn| Box::pin(async move {
    ///     Order::insert_into(txn).await?;
    ///     Ok(())
    /// })).await?;
    /// ```
    pub async fn transaction<F, T>(&self, f: F) -> Result<T, sea_orm::DbErr>
    where
        F: for<'a> FnOnce(
                &'a sea_orm::DatabaseTransaction,
            ) -> std::pin::Pin<
                Box<dyn Future<Output = Result<T, sea_orm::DbErr>> + Send + 'a>,
            > + Send,
        T: Send,
    {
        let conn = self.connect().await?;
        let txn = conn.begin().await?;
        match f(&txn).await {
            Ok(value) => {
                txn.commit().await?;
                Ok(value)
            }
            Err(e) => {
                let _ = txn.rollback().await;
                Err(e)
            }
        }
    }

    /// Begin a new database transaction and return a handle to it.
    ///
    /// The caller is responsible for calling `commit()` or `rollback()`.
    ///
    /// ```rust,ignore
    /// let txn = db.begin_transaction().await?;
    /// // ... run queries via `txn` (it implements `ConnectionTrait`) ...
    /// txn.commit().await?;
    /// ```
    pub async fn begin_transaction(&self) -> Result<sea_orm::DatabaseTransaction, sea_orm::DbErr> {
        let conn = self.connect().await?;
        conn.begin().await
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
                if db.database == ":memory:" {
                    "sqlite::memory:".to_string()
                } else {
                    format!("sqlite://{}?mode=rwc", db.database)
                }
            }
            _ => {
                if db.database == ":memory:" {
                    "sqlite::memory:".to_string()
                } else {
                    format!("sqlite://{}?mode=rwc", db.database)
                }
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

    pub async fn migrate_rollback<M: MigratorTrait>(
        &self,
        steps: Option<u32>,
    ) -> Result<(), sea_orm::DbErr> {
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

    // -----------------------------------------------------------------------
    // #[seeder] macro tests
    // -----------------------------------------------------------------------

    use larastvel_macros::seeder;

    #[seeder("custom_name")]
    struct CustomSeeder;

    impl CustomSeeder {
        async fn seed(_conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    #[seeder]
    struct AutoNameSeeder;

    impl AutoNameSeeder {
        async fn seed(_conn: &DbConn) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    #[test]
    fn test_seeder_macro_custom_name() {
        assert_eq!(CustomSeeder::name(), "custom_name");
    }

    #[test]
    fn test_seeder_macro_auto_name() {
        assert_eq!(AutoNameSeeder::name(), "auto_name_seeder");
    }

    #[test]
    fn test_seeder_macro_implements_trait() {
        // Verify the type satisfies Seeder bounds at compile time
        fn assert_seeder<S: Seeder>() {}
        assert_seeder::<CustomSeeder>();
        assert_seeder::<AutoNameSeeder>();
    }

    // -----------------------------------------------------------------------
    // Transaction tests
    // -----------------------------------------------------------------------

    use sea_orm::{ConnectionTrait, Statement};

    fn sqlite_manager() -> DatabaseManager {
        let mut config = Config::default();
        config.database.driver = "sqlite".to_string();
        config.database.database = ":memory:".to_string();
        DatabaseManager::new(&config)
    }

    #[tokio::test]
    async fn test_transaction_commits() {
        let db = sqlite_manager();
        db.transaction(|txn| {
            Box::pin(async move {
                txn.execute(Statement::from_string(
                    sea_orm::DatabaseBackend::Sqlite,
                    "CREATE TABLE tx_items (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
                ))
                .await?;
                txn.execute(Statement::from_sql_and_values(
                    sea_orm::DatabaseBackend::Sqlite,
                    "INSERT INTO tx_items (name) VALUES (?1)",
                    ["committed".into()],
                ))
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();

        let conn = db.connection().await.unwrap();
        let row = conn
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM tx_items".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(count, 1, "committed transaction should persist the insert");
    }

    #[tokio::test]
    async fn test_transaction_rolls_back_on_error() {
        let db = sqlite_manager();
        db.connection()
            .await
            .unwrap()
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "CREATE TABLE tx_items (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
            ))
            .await
            .unwrap();

        let result = db
            .transaction(|txn| {
                Box::pin(async move {
                    txn.execute(Statement::from_sql_and_values(
                        sea_orm::DatabaseBackend::Sqlite,
                        "INSERT INTO tx_items (name) VALUES (?1)",
                        ["rolled_back".into()],
                    ))
                    .await?;
                    Err::<(), sea_orm::DbErr>(sea_orm::DbErr::Custom("boom".to_string()))
                })
            })
            .await;

        assert!(result.is_err(), "the error should propagate");

        let conn = db.connection().await.unwrap();
        let row = conn
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM tx_items".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(count, 0, "rolled-back transaction must not persist changes");
    }

    #[tokio::test]
    async fn test_begin_transaction_manual_commit() {
        let db = sqlite_manager();
        let txn = db.begin_transaction().await.unwrap();
        txn.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE TABLE tx_items (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
        ))
        .await
        .unwrap();
        txn.execute(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tx_items (name) VALUES (?1)",
            ["manual".into()],
        ))
        .await
        .unwrap();
        txn.commit().await.unwrap();

        let conn = db.connection().await.unwrap();
        let row = conn
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM tx_items".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_begin_transaction_manual_rollback() {
        let db = sqlite_manager();
        db.connection()
            .await
            .unwrap()
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "CREATE TABLE tx_items (id INTEGER PRIMARY KEY, name TEXT)".to_string(),
            ))
            .await
            .unwrap();

        let txn = db.begin_transaction().await.unwrap();
        txn.execute(Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tx_items (name) VALUES (?1)",
            ["aborted".into()],
        ))
        .await
        .unwrap();
        txn.rollback().await.unwrap();

        let conn = db.connection().await.unwrap();
        let row = conn
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM tx_items".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_transaction_sequential_commits() {
        let db = sqlite_manager();
        for name in ["first", "second"] {
            db.transaction(|txn| {
                Box::pin(async move {
                    txn.execute(Statement::from_string(
                        sea_orm::DatabaseBackend::Sqlite,
                        "CREATE TABLE IF NOT EXISTS tx_items (id INTEGER PRIMARY KEY, name TEXT)"
                            .to_string(),
                    ))
                    .await?;
                    txn.execute(Statement::from_sql_and_values(
                        sea_orm::DatabaseBackend::Sqlite,
                        "INSERT INTO tx_items (name) VALUES (?1)",
                        [name.into()],
                    ))
                    .await?;
                    Ok(())
                })
            })
            .await
            .unwrap();
        }

        let conn = db.connection().await.unwrap();
        let row = conn
            .query_one(Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT COUNT(*) FROM tx_items".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        let count: i64 = row.try_get_by_index(0).unwrap();
        assert_eq!(count, 2, "two sequential transactions should both commit");
    }
}
