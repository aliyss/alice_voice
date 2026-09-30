//! Episode queue of the librarian.
//!
//! A turn writes one episode and returns. A background worker reads the
//! oldest unread episode with the librarian model and writes the facts it
//! reads, so the cost of learning never reaches the user.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use uuid::Uuid;

use alice_core::dto::MemorySavedFactDto;

use crate::librarian::entity::{memory_edge, memory_episode, memory_node};
use crate::librarian::store::LibrarianStore;

impl LibrarianStore {
    /// Store one turn for the worker to read.
    ///
    /// The episode names the message of the turn when the daemon stored
    /// one, so the transcript can mark the message the memory learned
    /// from once the worker has read it.
    pub async fn enqueue_episode(
        &self,
        id: Uuid,
        message_id: Option<Uuid>,
        text: &str,
        reply: &str,
        intent: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        memory_episode::Entity::insert(memory_episode::ActiveModel {
            id: Set(id),
            message_id: Set(message_id),
            text: Set(text.to_string()),
            reply: Set(reply.to_string()),
            intent: Set(intent.map(str::to_string)),
            processed: Set(false),
            error: Set(None),
            attempts: Set(0),
            next_attempt_at: Set(None),
            created_at: Set(at.fixed_offset()),
        })
        .exec_without_returning(self.db())
        .await?;
        Ok(())
    }

    /// Read the oldest episode the worker may read now.
    ///
    /// An episode the worker must try again waits for the time the failure
    /// set, so a model server that is down is asked again later rather than
    /// on every pass of the loop.
    pub async fn next_episode(&self) -> Result<Option<memory_episode::Model>, DbErr> {
        let now = Utc::now().fixed_offset();
        memory_episode::Entity::find()
            .filter(memory_episode::Column::Processed.eq(false))
            .filter(
                Condition::any()
                    .add(memory_episode::Column::NextAttemptAt.is_null())
                    .add(memory_episode::Column::NextAttemptAt.lte(now)),
            )
            .order_by_asc(memory_episode::Column::CreatedAt)
            .limit(1)
            .one(self.db())
            .await
    }

    /// Mark one episode as read, with the reason it failed or null.
    ///
    /// The worker calls this when it read the turn and when it left the
    /// turn behind for good, so a failure that names a reason is the mark
    /// of an episode the memory will not read again.
    pub async fn finish_episode(&self, id: Uuid, error: Option<&str>) -> Result<(), DbErr> {
        let active = memory_episode::ActiveModel {
            id: Set(id),
            processed: Set(true),
            error: Set(error.map(str::to_string)),
            next_attempt_at: Set(None),
            ..Default::default()
        };
        memory_episode::Entity::update(active)
            .exec(self.db())
            .await?;
        Ok(())
    }

    /// Keep one episode for another attempt after the given time.
    ///
    /// A turn is worth keeping: the model server may be restarted, and the
    /// turn said something the memory would otherwise never learn. The
    /// episode stays in the queue with the reason of the failure and the
    /// time of the next attempt.
    pub async fn retry_episode(
        &self,
        episode: &memory_episode::Model,
        error: &str,
        wait_until: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let active = memory_episode::ActiveModel {
            id: Set(episode.id),
            processed: Set(false),
            error: Set(Some(error.to_string())),
            attempts: Set(episode.attempts + 1),
            next_attempt_at: Set(Some(wait_until.fixed_offset())),
            ..Default::default()
        };
        memory_episode::Entity::update(active)
            .exec(self.db())
            .await?;
        Ok(())
    }

    /// Count the episodes that wait for the worker, or the ones it read.
    pub async fn count_episodes(&self, processed: bool) -> Result<u64, DbErr> {
        memory_episode::Entity::find()
            .filter(memory_episode::Column::Processed.eq(processed))
            .count(self.db())
            .await
    }

    /// Read the facts the memory learned from each of the given messages.
    ///
    /// One read answers for a whole page of turns, because a conversation
    /// shows many of them at once. The fact carries the name of the concept
    /// it belongs to, so the transcript reports what the memory learned
    /// rather than only which row learned it. A message the memory has not
    /// read yet, or one it learned nothing from, is not in the map.
    pub async fn memory_of_messages(
        &self,
        messages: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<MemorySavedFactDto>>, DbErr> {
        let episodes = memory_episode::Entity::find()
            .filter(memory_episode::Column::MessageId.is_in(messages.to_vec()))
            .all(self.db())
            .await?;
        if episodes.is_empty() {
            return Ok(HashMap::new());
        }

        let touched: Vec<Uuid> = episodes.iter().map(|episode| episode.id).collect();
        let edges = memory_edge::Entity::find()
            .filter(memory_edge::Column::SourceEpisode.is_in(touched))
            .order_by_asc(memory_edge::Column::ValidAt)
            .all(self.db())
            .await?;

        // The name of a concept is read once per concept rather than once
        // per fact, because one turn teaches several facts of one concept.
        let mut names: HashMap<Uuid, String> = HashMap::new();
        for edge in &edges {
            if names.contains_key(&edge.subject_id) {
                continue;
            }
            if let Some(node) = memory_node::Entity::find_by_id(edge.subject_id)
                .one(self.db())
                .await?
            {
                names.insert(edge.subject_id, node.title);
            }
        }

        let mut learned: HashMap<Uuid, Vec<MemorySavedFactDto>> = HashMap::new();
        for episode in episodes {
            let Some(message_id) = episode.message_id else {
                continue;
            };
            let facts: Vec<MemorySavedFactDto> = edges
                .iter()
                .filter(|edge| edge.source_episode == Some(episode.id))
                .map(|edge| MemorySavedFactDto {
                    concept: names
                        .get(&edge.subject_id)
                        .cloned()
                        .unwrap_or_else(|| edge.subject_id.to_string()),
                    relation: edge.relation.clone(),
                    value: edge.value.clone(),
                })
                .collect();
            if !facts.is_empty() {
                learned.insert(message_id, facts);
            }
        }
        Ok(learned)
    }
}
