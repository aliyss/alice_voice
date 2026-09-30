//! SeaORM entity for the scripts that wait for approval.
//! One row is one script the model wrote for a message no intent matched.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "pending_script")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Conversation the turn belongs to, or null when the queue is off.
    pub conversation_id: Option<Uuid>,
    /// The message the model wrote the script for.
    pub request_text: String,
    /// One sentence about what the script does.
    pub summary: String,
    /// The shell script.
    pub script: String,
    /// How rough the script is on the machine, between 0 and 100.
    pub destructiveness: i32,
    /// The decision of the user: pending, approved, denied, or ran.
    pub status: String,
    /// Time the daemon stored the script.
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
