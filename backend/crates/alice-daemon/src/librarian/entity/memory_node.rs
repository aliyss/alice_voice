//! SeaORM entity for one memory node.
//! One row is one concept the librarian learned about the user.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memory_node")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The key that names the node, for example `user`.
    pub key: String,
    /// The name the memory shows.
    pub title: String,
    /// The body of the concept in plain text.
    pub body: String,
    /// Time the daemon stored the node.
    pub created_at: DateTimeWithTimeZone,
    /// Time the daemon last changed the node.
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
