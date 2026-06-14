use colored::*;

pub fn create_notifications_table() {
    let migrations_dir = std::path::Path::new("database/migrations");
    std::fs::create_dir_all(migrations_dir).unwrap();

    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    let version = format!("m{}", secs);
    let file_name = format!("{}_create_notifications_table", version);

    let migration_content = r#"use larastvel_core::sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Notifications::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Notifications::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Notifications::Type).string().not_null())
                    .col(
                        ColumnDef::new(Notifications::NotifiableType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::NotifiableId)
                            .big_unsigned()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Notifications::Data).text().not_null())
                    .col(                        ColumnDef::new(Notifications::ReadAt).timestamp().null())
                    .col(
                        ColumnDef::new(Notifications::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Notifications::UpdatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Add index on (notifiable_type, notifiable_id) for polymorphic lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_notifications_notifiable")
                    .table(Notifications::Table)
                    .col(Notifications::NotifiableType)
                    .col(Notifications::NotifiableId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Notifications::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Notifications {
    Table,
    Id,
    Type,
    NotifiableType,
    NotifiableId,
    Data,
    ReadAt,
    CreatedAt,
    UpdatedAt,
}
"#
    .to_string();

    let file_path = migrations_dir.join(format!("{}.rs", file_name));
    if file_path.exists() {
        eprintln!(
            "{}",
            format!(
                "Error: Migration '{}' already exists '{}'.",
                "create_notifications_table",
                file_path.display()
            )
            .red()
        );
        return;
    }

    std::fs::write(&file_path, migration_content).unwrap();

    // Register in mod.rs
    let mod_path = migrations_dir.join("mod.rs");
    let mut mod_content = if mod_path.exists() {
        std::fs::read_to_string(&mod_path).unwrap()
    } else {
        String::new()
    };
    mod_content.push_str(&format!("pub mod {};\n", file_name));
    std::fs::write(&mod_path, mod_content).unwrap();

    println!(
        "{}",
        format!(
            "✓ Notifications table migration created at '{}'.",
            file_path.display()
        )
        .green()
        .bold()
    );
    println!(
        "{}",
        "  Don't forget to register the migration in your migrator.".dimmed()
    );
}
