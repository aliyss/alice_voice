//! SeaORM entity for the phrases a user may say for an intent.
//! One row is one variation of the message an intent answers, for example
//! `launch firefox` for `open application`. The resolver reads them next
//! to the name of the intent.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "intent_example")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub intent_id: Uuid,
    pub text: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
