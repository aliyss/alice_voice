//! Queue service of the message store.
//! This module owns the intake queue of the messages that wait for handling.
//! A handled message moves to the conversation message store.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
};
use uuid::Uuid;

use alice_core::dto::ChatRoleDto;

use crate::queue::entity::queued_message;

pub mod entity;

/// Status of a queued message that waits for handling.
pub const STATUS_PENDING: &str = "pending";

/// Status of a queued message that the daemon moved to the message store.
pub const STATUS_HANDLED: &str = "handled";

/// One message that enters the queue.
#[derive(Clone, Debug)]
pub struct NewQueuedMessage {
    /// Stable identifier. The message keeps it after handling.
    pub id: Uuid,
    /// Conversation the message belongs to.
    pub conversation_id: Uuid,
    /// Speaker.
    pub role: ChatRoleDto,
    /// Message text.
    pub text: String,
    /// Time the daemon accepted the message.
    pub created_at: DateTime<Utc>,
}

/// Service that stores the messages which wait for handling.
#[derive(Clone, Debug)]
pub struct QueueService {
    db: DatabaseConnection,
}

impl QueueService {
    /// Create a new queue service.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Add one message to the queue with the pending status.
    pub async fn enqueue(&self, message: &NewQueuedMessage) -> Result<(), sea_orm::DbErr> {
        let active = queued_message::ActiveModel {
            id: Set(message.id),
            conversation_id: Set(message.conversation_id),
            role: Set(message.role.as_str().to_string()),
            text: Set(message.text.clone()),
            status: Set(STATUS_PENDING.to_string()),
            created_at: Set(message.created_at.into()),
            handled_at: Set(None),
        };
        queued_message::Entity::insert(active)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// Move one queued message out of the pending status.
    pub async fn mark_handled(
        &self,
        id: Uuid,
        handled_at: DateTime<Utc>,
    ) -> Result<(), sea_orm::DbErr> {
        let active = queued_message::ActiveModel {
            id: Set(id),
            status: Set(STATUS_HANDLED.to_string()),
            handled_at: Set(Some(handled_at.into())),
            ..Default::default()
        };
        queued_message::Entity::update(active)
            .exec(&self.db)
            .await?;
        Ok(())
    }

    /// Count the messages that still wait for handling.
    #[allow(dead_code)]
    pub async fn count_pending(&self) -> Result<u64, sea_orm::DbErr> {
        queued_message::Entity::find()
            .filter(queued_message::Column::Status.eq(STATUS_PENDING))
            .count(&self.db)
            .await
    }
}
