use larastvel_core::sea_orm_migration::prelude::*;

use super::migrations;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(
            migrations::m20220101_000001_create_users_table::Migration,
        )]
    }
}
