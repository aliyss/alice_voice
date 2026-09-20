//! SeaORM entity for the handled messages of a conversation.
//! One row is one turn that the daemon moved out of the queue.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "chat_message")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub text: String,
    pub created_at: DateTimeWithTimeZone,
    pub intent_id: Option<String>,
    pub confidence: Option<f32>,
    /// How the daemon read the turn, as a JSON object.
    pub meta: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
