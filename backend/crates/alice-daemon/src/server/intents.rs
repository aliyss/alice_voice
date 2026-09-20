//! Intent handlers of the daemon.
//! One function handles one intent endpoint.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use tracing::instrument;
use uuid::Uuid;

use alice_core::dto::{
    IntentEntityWriteDto, IntentListDto, IntentWriteDto, ScriptPreviewDto, ScriptPreviewWriteDto,
};

use crate::intent::defaults;
use crate::intent::input::{EntityInput, IntentInput};
use crate::intent::SaveIntentError;
use crate::server::reply::{database_failed, error_reply, not_found, RestError};
use crate::server::state::AppState;

/// Reply of `GET /api/v1/intents`.
#[utoipa::path(get, path = "/api/v1/intents", tag = "intents", responses((status = 200, description = "Intent list", body = IntentListDto)))]
#[instrument(skip(state))]
pub async fn list_intents_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, RestError> {
    let items = state.stores.intents.list_intents().await.map_err(|err| {
        database_failed(err, "intent list", "The daemon could not list the intents.")
    })?;
    Ok(Json(IntentListDto { items }))
}

/// Reply of `POST /api/v1/intents`.
#[utoipa::path(
    post,
    path = "/api/v1/intents",
    tag = "intents",
    request_body = IntentWriteDto,
    responses(
        (status = 201, description = "Created intent", body = alice_core::dto::IntentDto),
        (status = 400, description = "Invalid input"),
        (status = 409, description = "The name is already in use")
    )
)]
#[instrument(skip(state, body))]
pub async fn create_intent_handler(
    State(state): State<AppState>,
    Json(body): Json<IntentWriteDto>,
) -> Result<impl IntoResponse, RestError> {
    let intent = state
        .stores
        .intents
        .create_intent(to_input(body), Utc::now())
        .await
        .map_err(map_write_error)?;
    Ok((StatusCode::CREATED, Json(intent)))
}

/// Reply of `PUT /api/v1/intents/{id}`.
#[utoipa::path(
    put,
    path = "/api/v1/intents/{id}", tag = "intents",
    params(("id" = Uuid, Path, description = "Intent id")),
    request_body = IntentWriteDto,
    responses(
        (status = 200, description = "Updated intent", body = alice_core::dto::IntentDto),
        (status = 400, description = "Invalid input"),
        (status = 404, description = "Intent not found"),
        (status = 409, description = "The name is already in use")
    )
)]
#[instrument(skip(state, body))]
pub async fn update_intent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<IntentWriteDto>,
) -> Result<impl IntoResponse, RestError> {
    let intent = state
        .stores
        .intents
        .update_intent(id, to_input(body), Utc::now())
        .await
        .map_err(map_write_error)?;
    Ok(Json(intent))
}

/// Reply of `DELETE /api/v1/intents/{id}`.
#[utoipa::path(
    delete,
    path = "/api/v1/intents/{id}",
    tag = "intents",
    params(("id" = Uuid, Path, description = "Intent id")),
    responses((status = 204, description = "Deleted"), (status = 404, description = "Intent not found"))
)]
#[instrument(skip(state))]
pub async fn delete_intent_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let removed = state
        .stores
        .intents
        .delete_intent(id)
        .await
        .map_err(map_write_error)?;
    if !removed {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Reply of `POST /api/v1/intents/script/preview`.
///
/// The settings page runs the script of an entity here, so a user reads
/// the values a script answers with before a turn reads them.
#[utoipa::path(
    post,
    path = "/api/v1/intents/script/preview", tag = "intents",
    request_body = ScriptPreviewWriteDto,
    responses((status = 200, description = "The values the script answered with", body = ScriptPreviewDto))
)]
#[instrument(skip(state, body))]
pub async fn preview_script_handler(
    State(state): State<AppState>,
    Json(body): Json<ScriptPreviewWriteDto>,
) -> Result<impl IntoResponse, RestError> {
    let preview: ScriptPreviewDto = state.stores.scripts.preview(&body.script).await;
    Ok(Json(preview))
}

/// Reply of `POST /api/v1/intents/examples`.
///
/// The endpoint adds the example intents that this catalog does not hold
/// yet and answers with the whole catalog, so the settings page shows the
/// new intents without a second read.
#[utoipa::path(post, path = "/api/v1/intents/examples", tag = "intents", responses((status = 200, description = "The whole catalog after the write", body = IntentListDto)))]
#[instrument(skip(state))]
pub async fn create_examples_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, RestError> {
    defaults::add_missing(&state.stores.intents, Utc::now())
        .await
        .map_err(map_write_error)?;
    let items = state.stores.intents.list_intents().await.map_err(|err| {
        database_failed(err, "intent list", "The daemon could not list the intents.")
    })?;
    Ok(Json(IntentListDto { items }))
}

/// Read the body of an intent write.
fn to_input(body: IntentWriteDto) -> IntentInput {
    IntentInput {
        name: body.name,
        description: body.description,
        command: body.command,
        entities: body.entities.into_iter().map(to_entity_input).collect(),
        examples: body.examples,
    }
}

/// Read one entity of an intent write.
fn to_entity_input(body: IntentEntityWriteDto) -> EntityInput {
    EntityInput {
        name: body.name,
        kind: body.kind,
        values: body.values,
        script: body.script,
        required: body.required,
    }
}

/// Map a write failure to a reply for the user.
fn map_write_error(error: SaveIntentError) -> RestError {
    match error {
        SaveIntentError::Invalid { reason } => error_reply(StatusCode::BAD_REQUEST, &reason),
        SaveIntentError::NameTaken { .. } => error_reply(
            StatusCode::CONFLICT,
            "An intent with that name already exists.",
        ),
        SaveIntentError::NotFound { .. } => not_found(),
        SaveIntentError::Database(err) => {
            database_failed(err, "intent write", "The daemon could not save the intent.")
        }
    }
}
