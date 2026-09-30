//! SeaORM entity for one memory episode.
//! One row is one turn the background worker has to read.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memory_episode")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// The message of the turn, or null for an episode of no stored turn.
    ///
    /// The transcript marks the message the memory learned from, so an
    /// episode names the message it came from. An episode the daemon
    /// stored for a turn it did not keep names no message.
    pub message_id: Option<Uuid>,
    /// The message of the turn.
    pub text: String,
    /// The reply of the daemon.
    pub reply: String,
    /// The intent of the turn, or null when none matched.
    pub intent: Option<String>,
    /// Whether the worker read the episode.
    pub processed: bool,
    /// Why the worker failed to read the episode, or null.
    pub error: Option<String>,
    /// Number of times the worker tried to read the episode.
    ///
    /// A model server that does not answer teaches nothing, and a turn is
    /// worth keeping, so the worker tries again a few times before it
    /// leaves the episode behind.
    pub attempts: i32,
    /// Time the worker may try the episode again, or null for now.
    pub next_attempt_at: Option<DateTimeWithTimeZone>,
    /// Time the daemon stored the episode.
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
