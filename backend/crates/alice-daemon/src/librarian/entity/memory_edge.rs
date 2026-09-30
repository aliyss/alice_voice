//! SeaORM entity for one dated memory edge.
//! One row is one relation of one memory node.
//!
//! The edge is temporal: a new value closes the older edge with the time
//! it stopped being true and never deletes it, so the memory answers a
//! question about the past as well as one about now.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memory_edge")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The node the relation belongs to.
    pub subject_id: Uuid,
    /// The relation the edge names, for example `lives_in`.
    pub relation: String,
    /// The value the edge carries.
    pub value: String,
    /// How sure the reader was of the fact, from zero to one.
    ///
    /// The reader answers this beside the fact, so an explicit statement
    /// of the user outranks a detail the assistant turned up.
    pub confidence: f32,
    /// How much the fact is worth, from zero to one.
    ///
    /// The reader answers this beside the fact. A name or a preference
    /// outranks a small detail, so the seed of a turn shows the fact that
    /// matters when it cannot show every fact.
    pub importance: f32,
    /// Number of times a later turn taught the same fact again.
    ///
    /// A fact the user said once may be a slip, so a fact a later turn
    /// confirms ranks above one nothing ever repeated.
    pub confirmations: i32,
    /// Time the memory last saw the fact, in ISO 8601.
    ///
    /// The first write sets it to the time the fact became true, and a
    /// confirmation moves it. The seed of a turn reports the age of a fact
    /// it shows, because an old fact is one the model should question.
    pub last_confirmed_at: DateTimeWithTimeZone,
    /// Time the edge became true.
    pub valid_at: DateTimeWithTimeZone,
    /// Time the edge stopped being true, or null while it is current.
    pub invalid_at: Option<DateTimeWithTimeZone>,
    /// The edge that replaced this one, or null while it is current.
    pub superseded_by: Option<Uuid>,
    /// The episode the librarian read the edge from, or null.
    pub source_episode: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
