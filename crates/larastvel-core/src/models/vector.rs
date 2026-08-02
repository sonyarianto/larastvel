use sea_orm::sea_query::{BinOper, Expr};
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, Select};

/// Distance metric for vector similarity searches, mirroring Laravel 13's
/// `whereVectorSimilarTo($column, $embedding, $distance)` options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VectorDistance {
    /// Cosine distance (`<=>`) — the Laravel default.
    #[default]
    Cosine,
    /// Euclidean (L2) distance (`<->`).
    L2,
    /// Inner product (`<#>`).
    InnerProduct,
}

impl VectorDistance {
    pub fn operator(self) -> &'static str {
        match self {
            VectorDistance::Cosine => "<=>",
            VectorDistance::L2 => "<->",
            VectorDistance::InnerProduct => "<#>",
        }
    }
}

/// Query-builder extension providing vector similarity clauses, mirroring
/// Laravel 13's semantic search API backed by PostgreSQL + `pgvector`.
///
/// ```rust,ignore
/// let documents = Document::find()
///     .where_vector_similar_to(document::Column::Embedding, "[0.1, 0.2, 0.3]")
///     .limit(10)
///     .all(db)
///     .await?;
/// ```
pub trait VectorSimilarityQuery: Sized {
    /// Filter rows by vector similarity to `embedding`, ordered by ascending
    /// distance (closest first).
    fn where_vector_similar_to<C>(self, column: C, embedding: &str) -> Self
    where
        C: ColumnTrait;

    /// Filter rows using a specific distance metric.
    fn where_vector_similar_with_distance<C>(
        self,
        column: C,
        embedding: &str,
        distance: VectorDistance,
    ) -> Self
    where
        C: ColumnTrait;
}

impl<E> VectorSimilarityQuery for Select<E>
where
    E: sea_orm::EntityTrait,
{
    fn where_vector_similar_to<C>(self, column: C, embedding: &str) -> Self
    where
        C: ColumnTrait,
    {
        self.where_vector_similar_with_distance(column, embedding, VectorDistance::Cosine)
    }

    fn where_vector_similar_with_distance<C>(
        self,
        column: C,
        embedding: &str,
        distance: VectorDistance,
    ) -> Self
    where
        C: ColumnTrait,
    {
        let operator = distance.operator();
        let expr = || Expr::col(column).binary(BinOper::Custom(operator), Expr::val(embedding));
        self.filter(expr()).order_by_asc(expr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::entity::prelude::*;
    use sea_orm::{DatabaseBackend, QuerySelect, QueryTrait};

    mod document {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "documents")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub title: String,
            pub embedding: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    #[test]
    fn test_where_vector_similar_to_builds_cosine_sql() {
        let select = document::Entity::find()
            .where_vector_similar_to(document::Column::Embedding, "[0.1, 0.2]");
        let stmt = select.build(DatabaseBackend::Postgres);
        let sql = &stmt.sql;
        let values = stmt.values.unwrap();

        assert!(sql.contains("\"embedding\" <=> $1"), "sql: {sql}");
        assert!(
            sql.contains("ORDER BY \"embedding\" <=> $2 ASC"),
            "sql: {sql}"
        );
        assert_eq!(values.0.len(), 2);
    }

    #[test]
    fn test_where_vector_similar_with_l2_distance() {
        let select = document::Entity::find().where_vector_similar_with_distance(
            document::Column::Embedding,
            "[0.1, 0.2]",
            VectorDistance::L2,
        );
        let stmt = select.build(DatabaseBackend::Postgres);
        let sql = &stmt.sql;

        assert!(sql.contains("\"embedding\" <-> $1"), "sql: {sql}");
        assert!(
            sql.contains("ORDER BY \"embedding\" <-> $2 ASC"),
            "sql: {sql}"
        );
    }

    #[test]
    fn test_where_vector_similar_with_inner_product() {
        let select = document::Entity::find().where_vector_similar_with_distance(
            document::Column::Embedding,
            "[0.1, 0.2]",
            VectorDistance::InnerProduct,
        );
        let stmt = select.build(DatabaseBackend::Postgres);
        let sql = &stmt.sql;

        assert!(sql.contains("\"embedding\" <#> $1"), "sql: {sql}");
    }

    #[test]
    fn test_vector_distance_operators() {
        assert_eq!(VectorDistance::Cosine.operator(), "<=>");
        assert_eq!(VectorDistance::L2.operator(), "<->");
        assert_eq!(VectorDistance::InnerProduct.operator(), "<#>");
        assert_eq!(VectorDistance::default(), VectorDistance::Cosine);
    }

    #[test]
    fn test_build_keeps_other_clauses() {
        let select = document::Entity::find()
            .filter(document::Column::Title.contains("wine"))
            .where_vector_similar_to(document::Column::Embedding, "[0.3, 0.4]")
            .limit(10);
        let stmt = select.build(DatabaseBackend::Postgres);
        let sql = &stmt.sql;
        let values = stmt.values.unwrap();

        assert!(sql.contains("WHERE"), "sql: {sql}");
        assert!(sql.contains("(\"embedding\" <=> $2)"), "sql: {sql}");
        assert!(
            sql.contains("ORDER BY \"embedding\" <=> $3 ASC"),
            "sql: {sql}"
        );
        assert!(sql.contains("LIMIT"), "sql: {sql}");
        assert_eq!(values.0.len(), 4);
    }
}
