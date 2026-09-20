//! Conversation handlers of the daemon.
//! One function handles one conversation endpoint.

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use tracing::instrument;
use uuid::Uuid;

use alice_core::dto::{
    ChatMessageDto, ConversationDetailDto, ConversationListDto, ConversationQuery,
};

use crate::server::reply::{database_failed, not_found, RestError};
use crate::server::state::AppState;

/// Reply of `GET /api/v1/conversations`.
#[instrument(skip(state))]
pub async fn list_conversations_handler(
    State(state): State<AppState>,
    Query(query): Query<ConversationQuery>,
) -> Result<impl IntoResponse, RestError> {
    let limit = query.limit_within(
        state.config.queue.page_limit_default,
        state.config.queue.page_limit_max,
    );
    let items = state
        .stores
        .conversations
        .list_conversations(limit)
        .await
        .map_err(|err| {
            database_failed(
                err,
                "conversation list",
                "The daemon could not list the conversations.",
            )
        })?;

    Ok(Json(ConversationListDto { items }))
}

/// Reply of `GET /api/v1/conversations/{id}`.
#[instrument(skip(state))]
pub async fn get_conversation_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let context = "conversation read";
    let conversation = state
        .stores
        .conversations
        .find_conversation(id)
        .await
        .map_err(|err| {
            database_failed(err, context, "The daemon could not read the conversation.")
        })?
        .ok_or_else(not_found)?;

    let limit = state.config.queue.page_limit_max;
    let mut messages = state
        .stores
        .conversations
        .list_messages(id, limit)
        .await
        .map_err(|err| {
            database_failed(err, context, "The daemon could not read the conversation.")
        })?;
    fill_intent_names(&state, &mut messages).await;

    Ok(Json(ConversationDetailDto {
        conversation,
        messages,
    }))
}

/// Fill the name of the intent of every message that carries one.
///
/// A stored message keeps the identifier of its intent, not its name, so
/// the reader turns the identifiers of the whole page into names at once.
/// A name that is gone stays null and the message keeps its identifier.
async fn fill_intent_names(state: &AppState, messages: &mut [ChatMessageDto]) {
    let ids: Vec<String> = messages
        .iter()
        .filter_map(|message| message.intent_id.clone())
        .collect();
    if ids.is_empty() {
        return;
    }

    let names = match state.stores.intents.names_for(&ids).await {
        Ok(names) => names,
        Err(err) => {
            tracing::warn!(error = %err, "intent name read failed");
            return;
        }
    };

    for message in messages.iter_mut() {
        if let Some(id) = &message.intent_id {
            message.intent_name = names.get(id).cloned();
        }
    }
}
