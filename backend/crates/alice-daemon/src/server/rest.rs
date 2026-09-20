//! REST handlers of the daemon.
//! One function handles one endpoint.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use tracing::instrument;

use alice_core::dto::{
    ChatReplyDto, ChatRequestDto, DaemonStateDto, HealthDto, StatusDto, SystemEventPayloadDto,
};

use crate::chat::{self, StoreTurnError};
use crate::server::reply::{bad_request, database_failed, error_reply, RestError};
use crate::server::state::AppState;

/// Reply of `GET /api/health` and `GET /health`.
#[utoipa::path(get, path = "/api/health", tag = "system", responses((status = 200, description = "Health reply", body = HealthDto)))]
#[instrument(skip(state))]
pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    health_reply(state).await
}

/// Alias of `GET /api/health` at `GET /health`.
#[utoipa::path(get, path = "/health", tag = "system", responses((status = 200, description = "Health reply", body = HealthDto)))]
#[instrument(skip(state))]
pub async fn health_alias_handler(State(state): State<AppState>) -> impl IntoResponse {
    health_reply(state).await
}

/// Build the health reply.
async fn health_reply(state: AppState) -> impl IntoResponse {
    let body = HealthDto {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let _ = &state;
    Json(body)
}

/// Reply of `GET /api/v1/status`.
#[utoipa::path(get, path = "/api/v1/status", tag = "system", responses((status = 200, description = "Status reply", body = StatusDto)))]
#[instrument(skip(state))]
pub async fn status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let body = StatusDto {
        state: DaemonStateDto::Idle,
        since: state.started_at,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    Json(body)
}

/// Reply of `POST /api/v1/chat`.
///
/// The daemon handles the message while the request runs: it resolves the
/// intent, runs its command, and answers. Every stage streams on the
/// socket stream, so the interface shows the turn while it runs.
///
/// The daemon stores the turn in a conversation when the queue is on. It
/// answers without a store when the queue is off.
#[utoipa::path(
    post,
    path = "/api/v1/chat", tag = "chat",
    request_body = ChatRequestDto,
    responses(
        (status = 201, description = "Handled turn", body = ChatReplyDto),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Conversation not found"),
        (status = 413, description = "Message too long")
    )
)]
#[instrument(skip(state, body))]
pub async fn chat_handler(
    State(state): State<AppState>,
    Json(body): Json<ChatRequestDto>,
) -> Result<impl IntoResponse, RestError> {
    // 1. Validate
    let text = validate_text(&body.text, state.config.queue.max_message_len)?;

    // 2. Store the turn, or build the ephemeral turn when the queue is off.
    let queue_enabled = state
        .stores
        .settings
        .get_queue_enabled()
        .await
        .unwrap_or(state.config.queue.enabled);
    let deps = state.handling_deps();
    let turn = if queue_enabled {
        chat::store_turn(
            &state.stores.conversations,
            &state.stores.queue,
            &deps,
            text,
            body.conversation_id,
        )
        .await
        .map_err(map_store_error)?
    } else {
        chat::build_ephemeral_turn(&deps, &text).await
    };

    // 3. Publish the reply event for the socket stream. The queued event
    //    left when the daemon accepted the message, so the stream shows
    //    the start of a turn before the stages that belong to it.
    publish_reply(&state, &turn);

    // 4. Return
    Ok((StatusCode::CREATED, Json(turn)))
}

/// Validate the text of a message and return the trimmed text.
///
/// The chat and the preview read the same field of the same length, so
/// both refuse a message the other would refuse and with the same words.
pub(crate) fn validate_text(text: &str, max_len: usize) -> Result<String, RestError> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err(bad_request("Type a message first."));
    }
    if trimmed.len() > max_len {
        return Err(error_reply(
            StatusCode::PAYLOAD_TOO_LARGE,
            "The message is too long.",
        ));
    }
    Ok(trimmed)
}

/// Map a store turn failure to a reply for the user.
fn map_store_error(error: StoreTurnError) -> RestError {
    match error {
        StoreTurnError::ConversationNotFound { .. } => {
            error_reply(StatusCode::NOT_FOUND, "The conversation does not exist.")
        }
        StoreTurnError::Database(err) => {
            database_failed(err, "chat store", "The daemon could not store the message.")
        }
    }
}

/// Publish the reply event of one handled turn.
fn publish_reply(state: &AppState, turn: &ChatReplyDto) {
    state
        .events
        .publish(SystemEventPayloadDto::MessageReplied { id: turn.reply.id });
}
