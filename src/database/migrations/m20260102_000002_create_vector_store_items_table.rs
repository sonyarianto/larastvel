use larastvel_core::sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS vector")
            .await?;
        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS vector_store_items (
                id BIGSERIAL PRIMARY KEY,
                file_path TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding vector(1536) NOT NULL,
                additional_arguments JSONB DEFAULT '{}'::jsonb
            )
            "#,
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS vector_store_items_embedding_idx \
             ON vector_store_items USING hnsw (embedding vector_cosine_ops)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VectorStoreItems::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum VectorStoreItems {
    Table,
}
