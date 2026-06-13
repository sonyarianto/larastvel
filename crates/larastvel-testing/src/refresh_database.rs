use larastvel_core::database::DatabaseManager;
use larastvel_core::sea_orm_migration::MigratorTrait;

#[async_trait::async_trait]
pub trait RefreshDatabase {
    type Migrator: MigratorTrait;

    async fn refresh_database(&self, db: &DatabaseManager) {
        db.migrate_fresh::<Self::Migrator>()
            .await
            .expect("Failed to refresh database");
    }
}
