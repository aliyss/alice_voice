//! SeaORM entity for one known name of a memory node.
//! One row is one key the memory has already seen for one concept.
//!
//! The reader is a model, so it names the same thing `user` one turn and
//! `flurin` the next. The memory writes the key of the node it resolved
//! such a name to, and keeps the name, so a later turn that uses it again
//! lands on the same concept instead of opening a second one.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memory_alias")]
pub struct Model {
    /// The known name, lower case, for example `flurin`.
    #[sea_orm(primary_key, auto_increment = false)]
    pub alias: String,
    /// The node the name belongs to.
    pub node_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
