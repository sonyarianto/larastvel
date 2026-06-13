//! Integration tests for `SoftDeletes` trait and global scopes (`scope_filter()`),
//! running against an in-memory SQLite database.
//!
//! All assertions live in a single test function because the global database
//! connection (`set_global_database`) uses a `OnceLock` that can only be set once.

use larastvel_core::models::{DbModel, SoftDeletes};
use larastvel_core::sea_orm;
use larastvel_core::sea_orm::entity::prelude::*;
use larastvel_core::sea_orm::{
    ActiveModelTrait, Condition, EntityTrait, QueryFilter, Set,
};
use larastvel_core::sea_orm_migration;
use larastvel_core::sea_orm_migration::MigratorTrait;

// ---------------------------------------------------------------------------
// Entity definition
// ---------------------------------------------------------------------------

mod post {
    use larastvel_core::sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "posts")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub title: String,
        pub deleted_at: Option<chrono::NaiveDateTime>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

// ---------------------------------------------------------------------------
// Migration
// ---------------------------------------------------------------------------

#[derive(sea_orm_migration::prelude::DeriveMigrationName)]
struct Migration;

#[async_trait::async_trait]
impl sea_orm_migration::prelude::MigrationTrait for Migration {
    async fn up(&self, manager: &sea_orm_migration::SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                sea_orm_migration::prelude::Table::create()
                    .table(Posts::Table)
                    .if_not_exists()
                    .col(
                        sea_orm_migration::prelude::ColumnDef::new(Posts::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        sea_orm_migration::prelude::ColumnDef::new(Posts::Title)
                            .string()
                            .not_null(),
                    )
                    .col(
                        sea_orm_migration::prelude::ColumnDef::new(Posts::DeletedAt)
                            .date_time()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &sea_orm_migration::SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                sea_orm_migration::prelude::Table::drop()
                    .table(Posts::Table)
                    .to_owned(),
            )
            .await
    }
}

enum Posts {
    Table,
    Id,
    Title,
    DeletedAt,
}

impl sea_orm_migration::prelude::Iden for Posts {
    fn unquoted(&self, write: &mut dyn std::fmt::Write) {
        write!(
            write,
            "{}",
            match self {
                Self::Table => "posts",
                Self::Id => "id",
                Self::Title => "title",
                Self::DeletedAt => "deleted_at",
            }
        )
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// Migrator
// ---------------------------------------------------------------------------

struct TestMigrator;

#[async_trait::async_trait]
impl sea_orm_migration::MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn sea_orm_migration::prelude::MigrationTrait>> {
        vec![Box::new(Migration)]
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

pub struct Post;

impl DbModel for Post {
    type Entity = post::Entity;

    /// Global scope: exclude soft-deleted records from all queries.
    fn scope_filter() -> Condition {
        Condition::all().add(post::Column::DeletedAt.is_null())
    }
}

impl SoftDeletes for Post {
    fn only_trashed() -> sea_orm::Select<post::Entity> {
        <post::Entity as EntityTrait>::find()
            .filter(post::Column::DeletedAt.is_not_null())
    }

    fn trashed(model: &post::Model) -> bool {
        model.deleted_at.is_some()
    }
}

// ---------------------------------------------------------------------------
// Tests — single test to avoid OnceLock global state issues
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_soft_deletes_with_global_scope() {
    // ---- setup ----
    let db = larastvel_core::sea_orm::Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    TestMigrator::up(&db, None)
        .await
        .expect("Migration failed");

    // Seed: 2 active + 2 soft-deleted posts
    use chrono::Utc;

    let active1 = post::ActiveModel {
        id: Set(1),
        title: Set("Active Post 1".to_string()),
        deleted_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("Insert active1 failed");

    let active2 = post::ActiveModel {
        id: Set(2),
        title: Set("Active Post 2".to_string()),
        deleted_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("Insert active2 failed");

    let trashed1 = post::ActiveModel {
        id: Set(3),
        title: Set("Trashed Post 1".to_string()),
        deleted_at: Set(Some(Utc::now().naive_utc())),
    }
    .insert(&db)
    .await
    .expect("Insert trashed1 failed");

    let trashed2 = post::ActiveModel {
        id: Set(4),
        title: Set("Trashed Post 2".to_string()),
        deleted_at: Set(Some(Utc::now().naive_utc())),
    }
    .insert(&db)
    .await
    .expect("Insert trashed2 failed");

    let all = vec![active1, active2, trashed1, trashed2];

    // Set the global DB connection so DbModel::db() works
    larastvel_core::models::set_global_database(db)
        .expect("set_global_database should succeed once");

    // ---- test all() excludes soft-deleted ----
    let posts = Post::all().await.expect("all() failed");
    assert_eq!(posts.len(), 2, "Expected 2 active posts from all()");
    for p in &posts {
        assert!(
            p.deleted_at.is_none(),
            "Active post '{}' should have deleted_at = None",
            p.title
        );
    }

    // ---- test find() excludes soft-deleted ----
    let found_active = Post::find(all[0].id)
        .await
        .expect("find() failed")
        .expect("Should find active post by ID");
    assert_eq!(found_active.title, "Active Post 1");
    assert!(found_active.deleted_at.is_none());

    // find() a soft-deleted post should return None
    let found_trashed = Post::find(all[2].id)
        .await
        .expect("find() failed");
    assert!(
        found_trashed.is_none(),
        "Soft-deleted post should not be found via find() with global scope"
    );

    // ---- test with_trashed() includes all ----
    let with_trashed = Post::with_trashed()
        .all(Post::db())
        .await
        .expect("with_trashed() failed");
    assert_eq!(with_trashed.len(), 4, "Expected all 4 posts from with_trashed()");

    // ---- test only_trashed() returns only soft-deleted ----
    let only_trashed = Post::only_trashed()
        .all(Post::db())
        .await
        .expect("only_trashed() failed");
    assert_eq!(only_trashed.len(), 2, "Expected 2 soft-deleted posts");
    for p in &only_trashed {
        assert!(
            p.deleted_at.is_some(),
            "Expected only trashed posts, but '{}' is active",
            p.title
        );
    }

    // ---- test trashed() check ----
    assert!(!Post::trashed(&all[0]), "Active post should NOT be trashed");
    assert!(Post::trashed(&all[2]), "Trashed post SHOULD be trashed");

    // ---- test soft_delete() ----
    let mut active_to_soft_delete: post::ActiveModel = all[1].clone().into();
    active_to_soft_delete.deleted_at = Set(Some(Utc::now().naive_utc()));
    let updated = Post::soft_delete(active_to_soft_delete)
        .await
        .expect("soft_delete() failed");
    assert!(
        updated.deleted_at.is_some(),
        "soft_delete should set deleted_at"
    );

    // After soft-deleting active2, only active1 should remain
    let remaining = Post::all().await.expect("all() after soft_delete failed");
    assert_eq!(remaining.len(), 1, "Only 1 active post remaining");
    assert_eq!(
        remaining[0].id, all[0].id,
        "Remaining post should be Active Post 1"
    );

    // ---- test force_delete() ----
    let am: post::ActiveModel = all[0].clone().into();
    Post::force_delete(am)
        .await
        .expect("force_delete() failed");

    // Force-deleted post is permanently gone; soft-deleted post still exists.
    // Remaining: active2 (soft-deleted) + 2 originally trashed = 3 posts total.
    // all() with scope returns 0 since all remaining have deleted_at set.
    let after_force = Post::with_trashed()
        .all(Post::db())
        .await
        .expect("with_trashed() after force_delete failed");
    assert_eq!(after_force.len(), 3, "Expected 3 posts after force delete");
    assert!(
        !after_force.iter().any(|p| p.id == all[0].id),
        "Force-deleted post should not exist"
    );
    assert!(
        after_force.iter().any(|p| p.id == all[1].id),
        "Soft-deleted post should still exist in DB"
    );

    // all() excludes everything since all remaining records are soft-deleted
    let none_active = Post::all().await.expect("all() after force_delete failed");
    assert_eq!(none_active.len(), 0, "Expected 0 active posts after all deletions");
}
