use std::sync::OnceLock;

use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DeleteResult, EntityTrait, IntoActiveModel,
    PrimaryKeyTrait,
};

static GLOBAL_DB: OnceLock<DatabaseConnection> = OnceLock::new();

pub fn set_global_database(db: DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    GLOBAL_DB.set(db).map_err(|_| {
        sea_orm::DbErr::Custom("Global database connection already initialized".to_string())
    })
}

pub fn database() -> &'static DatabaseConnection {
    GLOBAL_DB
        .get()
        .expect("Global database connection not initialized. Call set_global_database() first.")
}

#[async_trait::async_trait]
pub trait DbModel: Send + Sync + Sized + 'static {
    type Entity: EntityTrait;

    fn db() -> &'static DatabaseConnection {
        database()
    }

    async fn find(
        id: impl Into<<<Self::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType>
            + Send,
    ) -> Result<Option<<Self::Entity as EntityTrait>::Model>, sea_orm::DbErr> {
        Self::Entity::find_by_id(id).one(Self::db()).await
    }

    async fn all() -> Result<Vec<<Self::Entity as EntityTrait>::Model>, sea_orm::DbErr> {
        Self::Entity::find().all(Self::db()).await
    }

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

    async fn delete(
        active_model: <Self::Entity as EntityTrait>::ActiveModel,
    ) -> Result<DeleteResult, sea_orm::DbErr>
    where
        <Self::Entity as EntityTrait>::ActiveModel: Send,
    {
        active_model.delete(Self::db()).await
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::entity::prelude::*;
    use super::DbModel;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "test_items")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    pub struct TestItem;

    impl DbModel for TestItem {
        type Entity = Entity;
    }

    #[test]
    fn test_db_model_impl_compiles() {
        fn _assert<T: DbModel>() {}
        _assert::<TestItem>();
    }

    #[test]
    fn test_find_accepts_primary_key() {
        let _ = TestItem::find(42);
    }
}
