//! Conversation service of the message store.
//! This module owns the conversations and their handled messages.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use alice_core::dto::{ChatMessageDto, ChatRoleDto, ConversationDto, MessageMetaDto};

use crate::conversation::entity::{conversation, message};
use crate::conversation::title::build_title;

/// One handled message that the service appends to a conversation.
#[derive(Clone, Debug)]
pub struct NewMessage {
    /// Stable identifier of the message.
    pub id: Uuid,
    /// Speaker.
    pub role: ChatRoleDto,
    /// Message text.
    pub text: String,
    /// Time the daemon stored the message.
    pub created_at: DateTime<Utc>,
    /// Resolved intent identifier, or null.
    pub intent_id: Option<String>,
    /// Name of the resolved intent, or null.
    pub intent_name: Option<String>,
    /// Resolver confidence between 0 and 1, or null.
    pub confidence: Option<f32>,
    /// How the daemon read the turn, or null when it read nothing.
    pub meta: Option<MessageMetaDto>,
}

/// Service that stores conversations and their handled messages.
#[derive(Clone, Debug)]
pub struct ConversationService {
    db: DatabaseConnection,
}

impl ConversationService {
    /// Create a new conversation service.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Create a conversation and take its title from `first_message`.
    pub async fn create_conversation(
        &self,
        first_message: &str,
        at: DateTime<Utc>,
    ) -> Result<ConversationDto, sea_orm::DbErr> {
        let id = Uuid::new_v4();
        let title = build_title(first_message);
        let active = conversation::ActiveModel {
            id: Set(id),
            title: Set(title.clone()),
            created_at: Set(at.into()),
            updated_at: Set(at.into()),
        };
        conversation::Entity::insert(active).exec(&self.db).await?;
        Ok(ConversationDto {
            id,
            title,
            created_at: at,
            updated_at: at,
        })
    }

    /// Find one conversation, or null when it does not exist.
    pub async fn find_conversation(
        &self,
        id: Uuid,
    ) -> Result<Option<ConversationDto>, sea_orm::DbErr> {
        let row = conversation::Entity::find_by_id(id).one(&self.db).await?;
        Ok(row.map(model_to_dto))
    }

    /// List conversations, most recently updated first.
    pub async fn list_conversations(
        &self,
        limit: u64,
    ) -> Result<Vec<ConversationDto>, sea_orm::DbErr> {
        let rows = conversation::Entity::find()
            .order_by_desc(conversation::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(model_to_dto).collect())
    }

    /// Append one handled message and touch the conversation.
    pub async fn append_message(
        &self,
        conversation_id: Uuid,
        new_message: NewMessage,
    ) -> Result<ChatMessageDto, sea_orm::DbErr> {
        let role = new_message.role.as_str().to_string();
        let active = message::ActiveModel {
            id: Set(new_message.id),
            conversation_id: Set(conversation_id),
            role: Set(role.clone()),
            text: Set(new_message.text.clone()),
            created_at: Set(new_message.created_at.into()),
            intent_id: Set(new_message.intent_id.clone()),
            confidence: Set(new_message.confidence),
            meta: Set(encode_meta(new_message.meta.as_ref())),
        };
        message::Entity::insert(active).exec(&self.db).await?;

        let touch = conversation::ActiveModel {
            id: Set(conversation_id),
            updated_at: Set(new_message.created_at.into()),
            ..Default::default()
        };
        conversation::Entity::update(touch).exec(&self.db).await?;

        Ok(ChatMessageDto {
            id: new_message.id,
            conversation_id: Some(conversation_id),
            role: new_message.role,
            text: new_message.text,
            created_at: new_message.created_at,
            intent_id: new_message.intent_id,
            intent_name: new_message.intent_name,
            confidence: new_message.confidence,
            meta: new_message.meta,
        })
    }

    /// List the newest handled messages of one conversation, oldest first.
    pub async fn list_recent_messages(
        &self,
        conversation_id: Uuid,
        limit: u64,
    ) -> Result<Vec<ChatMessageDto>, sea_orm::DbErr> {
        let mut rows = message::Entity::find()
            .filter(message::Column::ConversationId.eq(conversation_id))
            .order_by_desc(message::Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        rows.reverse();
        Ok(rows.into_iter().map(message_to_dto).collect())
    }

    /// List the handled messages of one conversation, oldest first.
    pub async fn list_messages(
        &self,
        conversation_id: Uuid,
        limit: u64,
    ) -> Result<Vec<ChatMessageDto>, sea_orm::DbErr> {
        let rows = message::Entity::find()
            .filter(message::Column::ConversationId.eq(conversation_id))
            .order_by_asc(message::Column::CreatedAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(rows.into_iter().map(message_to_dto).collect())
    }
}

/// Map a conversation model to a DTO.
fn model_to_dto(model: conversation::Model) -> ConversationDto {
    ConversationDto {
        id: model.id,
        title: model.title,
        created_at: model.created_at.into(),
        updated_at: model.updated_at.into(),
    }
}

/// Map a message model to a DTO.
/// The name of the intent is not on the row, so the caller fills it in.
fn message_to_dto(model: message::Model) -> ChatMessageDto {
    ChatMessageDto {
        id: model.id,
        conversation_id: Some(model.conversation_id),
        role: ChatRoleDto::from_stored(&model.role),
        text: model.text,
        created_at: model.created_at.into(),
        intent_id: model.intent_id,
        intent_name: None,
        confidence: model.confidence,
        meta: decode_meta(model.meta.as_deref()),
    }
}

/// Encode the turn metadata for the store.
///
/// The store keeps the metadata of one turn as one JSON object, because
/// the shape of a turn changes with the pipeline and not with the store.
fn encode_meta(meta: Option<&MessageMetaDto>) -> Option<String> {
    meta.and_then(|meta| match serde_json::to_string(meta) {
        Ok(encoded) => Some(encoded),
        Err(err) => {
            tracing::warn!(error = %err, "the turn metadata is not writable");
            None
        }
    })
}

/// Read the turn metadata of one stored message.
///
/// A row the daemon cannot read is not a failed read: the message keeps
/// its text and reports no metadata.
fn decode_meta(stored: Option<&str>) -> Option<MessageMetaDto> {
    let stored = stored?;
    match serde_json::from_str(stored) {
        Ok(meta) => Some(meta),
        Err(err) => {
            tracing::warn!(error = %err, "the stored turn metadata is not readable");
            None
        }
    }
}
