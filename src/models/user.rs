use larastvel_core::table;

#[table("users")]
pub struct User {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub password: String,
    pub email_verified_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}
