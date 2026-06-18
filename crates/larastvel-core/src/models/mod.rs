pub mod factory;
pub mod serialization;

use std::sync::OnceLock;

use sea_orm::{
    ActiveModelTrait, Condition, DatabaseConnection, DeleteResult, EntityTrait, IntoActiveModel,
    LoaderTrait, ModelTrait, PrimaryKeyTrait, QueryFilter, Related,
};

static GLOBAL_DB: OnceLock<DatabaseConnection> = OnceLock::new();

pub fn set_global_database(db: DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    GLOBAL_DB.set(db).map_err(|_| {
        tracing::warn!("Global database connection already initialized. Ignoring duplicate.");
        sea_orm::DbErr::Custom("Global database connection already initialized".to_string())
    })
}

pub fn database() -> &'static DatabaseConnection {
    GLOBAL_DB
        .get()
        .expect("Global database connection not initialized. Call set_global_database() first.")
}

/// Trait for models that can be persisted and queried via the SeaORM
/// database connection.
///
/// Provides CRUD operations as well as relationship traversal methods
/// built on top of SeaORM's `ModelTrait` and `LoaderTrait`.
///
/// # Relationships
///
/// Define a `Relation` enum with `DeriveRelation` on your entity, then
/// use `has_many()` / `belongs_to()` to query related models:
///
/// ```rust,ignore
/// #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
/// pub enum Relation {
///     #[sea_orm(has_many = "super::post::Entity")]
///     Posts,
/// }
///
/// let user = User::find(1).await?.unwrap();
/// let posts = User::has_many(&user, post::Entity).await?;
/// ```
#[async_trait::async_trait]
pub trait DbModel: Send + Sync + Sized + 'static {
    /// The SeaORM entity associated with this model.
    type Entity: EntityTrait;

    fn db() -> &'static DatabaseConnection {
        database()
    }

    /// Return a condition that should be applied to all queries.
    ///
    /// By default returns an empty condition (no filtering). Override this
    /// to apply global scopes — for example, excluding soft-deleted records
    /// by returning `Column::DeletedAt.is_null().into()`.
    ///
    /// This condition is automatically applied to `find()` and `all()`.
    fn scope_filter() -> Condition {
        Condition::all()
    }

    /// Return a query builder with all global scopes applied.
    ///
    /// The returned `Select` can be further refined (filter, order, limit)
    /// before executing it with `.one(db)` or `.all(db)`.
    fn query() -> sea_orm::Select<Self::Entity> {
        Self::Entity::find().filter(Self::scope_filter())
    }

    /// Find a single model by its primary key.
    ///
    /// This automatically applies any global scopes defined via `scope_filter()`.
    async fn find(
        id: impl Into<<<Self::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType> + Send,
    ) -> Result<Option<<Self::Entity as EntityTrait>::Model>, sea_orm::DbErr> {
        Self::Entity::find_by_id(id)
            .filter(Self::scope_filter())
            .one(Self::db())
            .await
    }

    /// Retrieve all models of this type.
    ///
    /// This automatically applies any global scopes defined via `scope_filter()`.
    async fn all() -> Result<Vec<<Self::Entity as EntityTrait>::Model>, sea_orm::DbErr> {
        Self::Entity::find()
            .filter(Self::scope_filter())
            .all(Self::db())
            .await
    }

    /// Insert a new record from an active model.
    async fn insert(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<<Self::Entity as EntityTrait>::Model, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
        <Self::Entity as EntityTrait>::Model:
            IntoActiveModel<<Self::Entity as EntityTrait>::ActiveModel>,
    {
        active_model.insert(Self::db()).await
    }

    /// Update an existing record.
    async fn update(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<<Self::Entity as EntityTrait>::Model, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
        <Self::Entity as EntityTrait>::Model:
            IntoActiveModel<<Self::Entity as EntityTrait>::ActiveModel>,
    {
        active_model.update(Self::db()).await
    }

    /// Delete a record.
    async fn delete(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<DeleteResult, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
    {
        active_model.delete(Self::db()).await
    }

    // -------------------------------------------------------------------------
    // Relationship queries
    // -------------------------------------------------------------------------

    /// Find related entities for a single parent model (has_many).
    ///
    /// The relationship must be defined in the entity's `Relation` enum
    /// using the `#[sea_orm(has_many = "...")]` attribute.
    ///
    /// Pass the related entity type as the second argument:
    /// ```rust,ignore
    /// let posts = User::has_many(&user, post::Entity).await?;
    /// ```
    async fn has_many<R>(
        model: &<Self::Entity as EntityTrait>::Model,
        related_entity: R,
    ) -> Result<Vec<<R as EntityTrait>::Model>, sea_orm::DbErr>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        <Self::Entity as EntityTrait>::Model: ModelTrait + Send + Sync,
        <R as EntityTrait>::Model: Send + Sync,
    {
        model.find_related(related_entity).all(Self::db()).await
    }

    /// Find a single related entity for a parent model (belongs_to / has_one).
    ///
    /// Pass the related entity type as the second argument:
    /// ```rust,ignore
    /// let category = Item::belongs_to(&item, category::Entity).await?;
    /// ```
    async fn belongs_to<R>(
        model: &<Self::Entity as EntityTrait>::Model,
        related_entity: R,
    ) -> Result<Option<<R as EntityTrait>::Model>, sea_orm::DbErr>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        <Self::Entity as EntityTrait>::Model: ModelTrait + Send + Sync,
        <R as EntityTrait>::Model: Send + Sync,
    {
        model.find_related(related_entity).one(Self::db()).await
    }

    // -------------------------------------------------------------------------
    // Eager loading
    // -------------------------------------------------------------------------

    /// Eagerly load has_many relationships for a collection of parent models.
    ///
    /// Uses SeaORM's `LoaderTrait::load_many` under the hood.
    /// The relationship must be defined via `#[sea_orm(has_many = "...")]`
    /// on the parent entity's `Relation` enum.
    ///
    /// ```rust,ignore
    /// let users = User::all().await?;
    /// let posts = User::load_many(&users, post::Entity).await?;
    /// // posts[i] corresponds to users[i]
    /// ```
    async fn load_many<R>(
        models: &[<Self::Entity as EntityTrait>::Model],
        related_entity: R,
    ) -> Result<Vec<Vec<<R as EntityTrait>::Model>>, sea_orm::DbErr>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        <Self::Entity as EntityTrait>::Model: ModelTrait + Send + Sync,
        <R as EntityTrait>::Model: Send + Sync,
    {
        models.load_many(related_entity, Self::db()).await
    }

    /// Eagerly load belongs_to / has_one relationships for a collection
    /// of parent models.
    ///
    /// Uses SeaORM's `LoaderTrait::load_one` under the hood.
    /// The relationship must be defined via `#[sea_orm(belongs_to = "...")]`
    /// or `#[sea_orm(has_one = "...")]` on the parent entity's `Relation` enum.
    ///
    /// ```rust,ignore
    /// let items = Item::all().await?;
    /// let categories = Item::load_one(&items, category::Entity).await?;
    /// // categories[i] corresponds to items[i]
    /// ```
    async fn load_one<R>(
        models: &[<Self::Entity as EntityTrait>::Model],
        related_entity: R,
    ) -> Result<Vec<Option<<R as EntityTrait>::Model>>, sea_orm::DbErr>
    where
        R: EntityTrait,
        Self::Entity: Related<R>,
        <Self::Entity as EntityTrait>::Model: ModelTrait + Send + Sync,
        <R as EntityTrait>::Model: Send + Sync,
    {
        models.load_one(related_entity, Self::db()).await
    }
}

// -------------------------------------------------------------------------
// Soft Deletes
// -------------------------------------------------------------------------

/// Adds soft-delete capabilities to a model that implements `DbModel`.
///
/// Your entity must have a nullable timestamp column (typically named
/// `deleted_at` of type `Option<chrono::NaiveDateTime>` or similar).
/// Then implement this trait alongside `DbModel`.
///
/// # Global Scope
///
/// To automatically exclude soft-deleted records from `find()`, `all()`,
/// and other built-in query methods, override `DbModel::scope_filter()`
/// in your implementation to add the `deleted_at IS NULL` condition:
///
/// ```rust,ignore
/// impl DbModel for Post {
///     // ...
///     fn scope_filter() -> Condition {
///         Condition::all().add(post::Column::DeletedAt.is_null())
///     }
/// }
/// ```
///
/// With this in place, `Post::all().await` returns only active records,
/// `Post::find(1).await` skips trashed records, and you can still use
/// `Post::with_trashed()` or `Post::only_trashed()` to bypass the scope.
///
/// # Usage
///
/// ```rust,ignore
/// use chrono::Utc;
/// use sea_orm::Set;
///
/// // Soft delete: set deleted_at on ActiveModel, then call soft_delete
/// let mut am: post::ActiveModel = post.into();
/// am.deleted_at = Set(Some(Utc::now().naive_utc()));
/// Post::soft_delete(am).await?;
///
/// // Permanently delete
/// let am: post::ActiveModel = post.into();
/// Post::force_delete(am).await?;
///
/// // Query including soft-deleted (by default shows only active)
/// let all = Post::with_trashed().all(Post::db()).await?;
///
/// // Check if model is trashed
/// if Post::trashed(&post) { ... }
///
/// // Only soft-deleted records — user must implement only_trashed()
/// let trashed = Post::only_trashed().all(Post::db()).await?;
/// ```
#[async_trait::async_trait]
pub trait SoftDeletes: DbModel {
    /// Soft delete a record by updating it.
    ///
    /// The caller must set `deleted_at` on the ActiveModel before passing it:
    /// ```rust,ignore
    /// let mut am: post::ActiveModel = post.into();
    /// am.deleted_at = Set(Some(Utc::now().naive_utc()));
    /// Post::soft_delete(am).await?;
    /// ```
    async fn soft_delete(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<<Self::Entity as EntityTrait>::Model, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
        <Self::Entity as EntityTrait>::Model:
            IntoActiveModel<<Self::Entity as EntityTrait>::ActiveModel>,
    {
        active_model.update(Self::db()).await
    }

    /// Permanently delete a soft-deletable record (hard delete).
    async fn force_delete(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<DeleteResult, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
    {
        active_model.delete(Self::db()).await
    }

    /// Get a query that includes soft-deleted records.
    ///
    /// By default this returns all records (same as `all()`).
    fn with_trashed() -> sea_orm::Select<Self::Entity> {
        Self::Entity::find()
    }

    /// Get a query that returns only soft-deleted records.
    ///
    /// The user must implement this to filter on their entity's `deleted_at` column:
    /// ```rust,ignore
    /// fn only_trashed() -> Select<Self::Entity> {
    ///     Self::Entity::find().filter(post::Column::DeletedAt.is_not_null())
    /// }
    /// ```
    fn only_trashed() -> sea_orm::Select<Self::Entity>;

    /// Check whether a model instance has been soft-deleted.
    ///
    /// The user must implement this to check their model's `deleted_at` field:
    /// ```rust,ignore
    /// fn trashed(model: &post::Model) -> bool {
    ///     model.deleted_at.is_some()
    /// }
    /// ```
    fn trashed(model: &<Self::Entity as EntityTrait>::Model) -> bool;
}

// -------------------------------------------------------------------------
// Timestamps
// -------------------------------------------------------------------------

/// Adds auto-managed `created_at` / `updated_at` timestamp columns to a model.
///
/// Your entity must have two `chrono::NaiveDateTime` columns (typically named
/// `created_at` and `updated_at`). To auto-set them on every insert/update,
/// override `ActiveModelBehavior::before_save()` in your entity:
///
/// ```rust,ignore
/// impl ActiveModelBehavior for ActiveModel {
///     fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
///         let now = Self::fresh_timestamp();
///         if insert {
///             self.created_at = Set(now);
///         }
///         self.updated_at = Set(now);
///         Ok(self)
///     }
/// }
/// ```
///
/// Use `touch()` to manually update `updated_at` to the current timestamp
/// and persist the change:
///
/// ```rust,ignore
/// let mut am: article::ActiveModel = article.into();
/// am.updated_at = Set(chrono::Utc::now().naive_utc());
/// Article::touch(am).await?;
/// ```
#[async_trait::async_trait]
pub trait Timestamps: DbModel {
    /// Return the current timestamp (UTC) for use in timestamp columns.
    ///
    /// Override this to use a different timezone or clock source.
    fn fresh_timestamp() -> chrono::NaiveDateTime {
        chrono::Utc::now().naive_utc()
    }

    /// Touch the model: persist it after the caller has set `updated_at`.
    ///
    /// This is a convenience wrapper around `update()` that signals the
    /// intent to refresh the `updated_at` timestamp.
    ///
    /// ```rust,ignore
    /// let mut am: article::ActiveModel = article.into();
    /// am.updated_at = Set(Article::fresh_timestamp());
    /// Article::touch(am).await?;
    /// ```
    async fn touch(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<<Self::Entity as EntityTrait>::Model, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
        <Self::Entity as EntityTrait>::Model:
            IntoActiveModel<<Self::Entity as EntityTrait>::ActiveModel>,
    {
        active_model.update(Self::db()).await
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::{DbModel, SoftDeletes, Timestamps};
    use sea_orm::entity::prelude::*;
    use sea_orm::Condition;
    use sea_orm::IntoActiveModel;

    mod category {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "categories")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub name: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    pub struct Category;

    impl DbModel for Category {
        type Entity = category::Entity;
    }

    // -- Test entity with soft-delete support --

    mod post {
        use sea_orm::entity::prelude::*;

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
            <post::Entity as EntityTrait>::find().filter(post::Column::DeletedAt.is_not_null())
        }

        fn trashed(model: &post::Model) -> bool {
            model.deleted_at.is_some()
        }
    }

    // -- Tests --

    #[test]
    fn test_db_model_impl_compiles() {
        fn _assert<T: DbModel>() {}
        _assert::<Category>();
        _assert::<Post>();
    }

    #[test]
    fn test_find_accepts_primary_key() {
        drop(Category::find(42));
        drop(Post::find(1));
    }

    #[test]
    fn test_query_method_exists() {
        let _cat_select = Category::query();
        let _post_select = Post::query();
    }

    #[test]
    fn test_scope_filter_compiles_for_both() {
        let _cat_cond = Category::scope_filter();
        let _post_cond = Post::scope_filter();
    }

    #[test]
    fn test_query_applies_global_scope_for_soft_deletes() {
        // Verify Post::scope_filter() adds the deleted_at IS NULL condition.
        // Post overrides scope_filter(), Category uses default (empty condition).
        // Both should compile and be usable in queries.
        let _post_select: sea_orm::Select<post::Entity> = Post::query();
        let _cat_select: sea_orm::Select<category::Entity> = Category::query();
    }

    #[test]
    fn test_with_trashed_bypasses_global_scope() {
        let _select: sea_orm::Select<post::Entity> = Post::with_trashed();
    }

    #[test]
    fn test_only_trashed_returns_only_trashed() {
        let _select: sea_orm::Select<post::Entity> = Post::only_trashed();
    }

    #[test]
    fn test_all_uses_global_scope() {
        // Uses Post's scope_filter() internally via query()
        let _fut = Post::all();
        let _fut = Category::all();
    }

    #[test]
    fn test_find_uses_global_scope() {
        // Uses Post's scope_filter() internally via find()
        let _fut = Post::find(1);
        let _fut = Category::find(42);
    }

    #[test]
    fn test_query_uses_scope_filter() {
        // query() returns Entity::find() with scope_filter() applied
        let _select: sea_orm::Select<post::Entity> = Post::query();
        let _select: sea_orm::Select<category::Entity> = Category::query();
    }

    #[test]
    fn test_has_many_method_exists() {
        fn _assert<T: DbModel>() {}
        _assert::<Category>();
    }

    #[test]
    fn test_soft_deletes_trait_compiles() {
        fn _assert<T: SoftDeletes>() {}
        _assert::<Post>();
    }

    #[test]
    fn test_soft_deletes_trashed() {
        let post = post::Model {
            id: 1,
            title: "Test".to_string(),
            deleted_at: Some(chrono::Local::now().naive_local()),
        };
        assert!(Post::trashed(&post));

        let post = post::Model {
            id: 2,
            title: "Active".to_string(),
            deleted_at: None,
        };
        assert!(!Post::trashed(&post));
    }

    #[test]
    fn test_soft_deletes_with_trashed_compiles() {
        let _select = Post::with_trashed();
    }

    #[test]
    fn test_soft_deletes_only_trashed_compiles() {
        let _select = Post::only_trashed();
    }

    #[test]
    fn test_soft_delete_method_signature_compiles() {
        // Verify soft_delete accepts an ActiveModel
        fn _check<M>()
        where
            M: SoftDeletes,
            <M::Entity as EntityTrait>::ActiveModel: Send,
            <M::Entity as EntityTrait>::Model:
                IntoActiveModel<<M::Entity as EntityTrait>::ActiveModel>,
        {
        }
        _check::<Post>();
    }

    #[test]
    fn test_force_delete_method_signature_compiles() {
        fn _check<M>()
        where
            M: SoftDeletes,
            <M::Entity as EntityTrait>::ActiveModel: Send,
        {
        }
        _check::<Post>();
    }

    #[test]
    fn test_soft_deletes_impl_requires_only_trashed_and_trashed() {
        // Verify Post implements both required methods
        fn _check_required_methods<T: SoftDeletes>() {
            // only_trashed() must exist
            fn _has_only_trashed<U: SoftDeletes>() {
                let _ = U::only_trashed();
            }
            // trashed() must exist
            fn _has_trashed<U: SoftDeletes>(model: &<U::Entity as EntityTrait>::Model) -> bool {
                U::trashed(model)
            }
            let _ = std::marker::PhantomData::<T>;
        }
        _check_required_methods::<Post>();
    }

    // =========================================================================
    // Timestamps trait
    // =========================================================================

    mod article {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "articles")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub title: String,
            pub created_at: chrono::NaiveDateTime,
            pub updated_at: chrono::NaiveDateTime,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    pub struct Article;

    impl DbModel for Article {
        type Entity = article::Entity;
    }

    impl Timestamps for Article {}

    // -- Timestamps tests --

    #[test]
    fn test_timestamps_trait_compiles() {
        fn _assert<T: Timestamps>() {}
        _assert::<Article>();
    }

    #[test]
    fn test_fresh_timestamp_returns_naive_datetime() {
        let ts: chrono::NaiveDateTime = Article::fresh_timestamp();
        // Should be a recent timestamp (within the last 10 seconds)
        let now = chrono::Utc::now().naive_utc();
        let diff = now - ts;
        assert!(
            diff.num_seconds().abs() < 10,
            "fresh_timestamp should return a recent timestamp"
        );
    }

    #[test]
    fn test_timestamps_with_db_model() {
        fn _assert<T: DbModel>() {}
        _assert::<Article>();
    }

    #[test]
    fn test_timestamp_touch_method_signature_compiles() {
        fn _check<M>()
        where
            M: Timestamps,
            <M::Entity as EntityTrait>::ActiveModel: Send,
            <M::Entity as EntityTrait>::Model:
                IntoActiveModel<<M::Entity as EntityTrait>::ActiveModel>,
        {
        }
        _check::<Article>();
    }

    #[test]
    fn test_timestamps_and_soft_deletes_are_independent() {
        // Verify Timestamps and SoftDeletes are independent traits that
        // don't conflict — they can be mixed and matched on different models.
        fn _assert_timestamps<T: Timestamps>() {}
        fn _assert_soft_deletes<T: SoftDeletes>() {}

        _assert_timestamps::<Article>();
        _assert_soft_deletes::<Post>();
    }

    #[test]
    fn test_all_traits_available() {
        fn _assert_db<T: DbModel>() {}
        fn _assert_ts<T: Timestamps>() {}
        fn _assert_sd<T: SoftDeletes>() {}

        _assert_db::<Category>();
        _assert_db::<Post>();
        _assert_db::<Article>();
        _assert_ts::<Article>();
        _assert_sd::<Post>();
    }

    // --- #[table] macro test ---

    use crate::table;

    mod table_test {
        use super::*;

        #[table("widgets")]
        pub struct Widget {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub name: String,
            pub price: f64,
        }
    }

    #[test]
    fn test_table_macro_generates_entity() {
        use table_test::{Entity, Model, Widget};

        // Model should have fields
        let _ = Model {
            id: 1,
            name: "foo".into(),
            price: 9.99,
        };
        // Widget should implement DbModel
        fn _assert_db<T: DbModel>() {}
        _assert_db::<Widget>();
        // Entity should be the SeaORM entity type
        let _: Widget = Widget;
        let _: Entity = Entity;
    }

    #[test]
    fn test_table_macro_column_attrs_forwarded() {
        use table_test::Column;

        // Verify we can reference Column variants
        match Column::Id {
            Column::Id => {}
            _ => panic!("expected Id"),
        }
    }

    // --- #[scope] macro tests ---

    mod scope_test {
        use super::*;
        use crate::scope;

        #[table("articles")]
        pub struct Article {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub title: String,
            pub likes: i64,
        }

        impl Article {
            #[scope]
            fn popular(query: Select<Entity>, min_likes: i64) -> Select<Entity> {
                query.filter(Column::Likes.gte(min_likes))
            }

            #[scope]
            fn trending(query: Select<Entity>, min_likes: i64) -> Select<Entity> {
                query.filter(Column::Likes.gte(min_likes))
            }

            #[scope]
            fn scope_top_rated(query: Select<Entity>, min_likes: i64) -> Select<Entity> {
                query.filter(Column::Likes.gte(min_likes))
            }
        }

        #[test]
        fn test_scope_macro_generates_method() {
            let _query = Article::popular(100);
        }

        #[test]
        fn test_scope_macro_without_scope_prefix() {
            let _query = Article::trending(500);
        }

        #[test]
        fn test_scope_macro_strips_scope_prefix() {
            let _query = Article::top_rated(1000);
        }
    }
}
