//! SeaORM entity for the entities of an intent.
//! One row is one value the intent reads out of a message. An entity is
//! open, closed, or script: a script entity carries the command that
//! provides its values. An entity is required or optional: the daemon
//! asks the user for the value of a required entity it cannot read, and
//! runs the command of an optional one without it.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "intent_entity")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub intent_id: Uuid,
    pub name: String,
    pub kind: String,
    /// Shell command that provides the values of a script entity.
    pub script: Option<String>,
    /// Whether the intent needs a value for this entity.
    pub required: bool,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
