//! Librarian handlers of the daemon.
//!
//! The settings page reads the state of the memory here and searches it,
//! and the script of a turn may write one node by hand. The write of a
//! turn itself happens in the background and never reaches a handler.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use alice_core::dto::{
    LibrarianLintDto, LibrarianStatusDto, MemoryMergeRequestDto, MemoryNodeDto, MemoryQueryDto,
    MemoryQueryRequestDto, MemoryWriteRequestDto,
};

use crate::server::reply::{bad_request, database_failed, not_found, RestError};
use crate::server::state::AppState;

/// The query of `GET /api/v1/librarian/memories`.
#[derive(Debug, Default, Deserialize)]
pub struct MemoryListQuery {
    /// The words to search the memory for.
    pub text: Option<String>,
    /// The largest number of nodes to return.
    pub limit: Option<u64>,
}

/// Reply of `GET /api/v1/librarian`.
#[utoipa::path(get, path = "/api/v1/librarian", tag = "librarian", responses((status = 200, description = "State of the librarian", body = LibrarianStatusDto)))]
#[instrument(skip(state))]
pub async fn get_librarian_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, RestError> {
    Ok(Json(read_status(&state).await?))
}

/// Read the state of the librarian.
pub async fn read_status(state: &AppState) -> Result<LibrarianStatusDto, RestError> {
    state.stores.librarian.status().await.map_err(|err| {
        database_failed(
            err,
            "librarian status",
            "The daemon could not read the librarian.",
        )
    })
}

/// Reply of `GET /api/v1/librarian/memories`.
#[utoipa::path(
    get,
    path = "/api/v1/librarian/memories",
    tag = "librarian",
    params(
        ("text" = Option<String>, Query, description = "Words to search the memory for"),
        ("limit" = Option<u64>, Query, description = "Largest number of nodes to return")
    ),
    responses((status = 200, description = "Memory nodes", body = MemoryQueryDto))
)]
#[instrument(skip(state))]
pub async fn list_memories_handler(
    State(state): State<AppState>,
    Query(query): Query<MemoryListQuery>,
) -> Result<impl IntoResponse, RestError> {
    let nodes = read_nodes(&state, query.text.unwrap_or_default(), query.limit).await?;
    Ok(Json(MemoryQueryDto { nodes }))
}

/// Reply of `POST /api/v1/librarian/query`.
#[utoipa::path(
    post,
    path = "/api/v1/librarian/query",
    tag = "librarian",
    request_body = MemoryQueryRequestDto,
    responses((status = 200, description = "Memory nodes", body = MemoryQueryDto))
)]
#[instrument(skip(state, body))]
pub async fn query_memory_handler(
    State(state): State<AppState>,
    Json(body): Json<MemoryQueryRequestDto>,
) -> Result<impl IntoResponse, RestError> {
    let nodes = read_nodes(&state, body.text, body.limit).await?;
    Ok(Json(MemoryQueryDto { nodes }))
}

/// Reply of `POST /api/v1/librarian/memories`.
#[utoipa::path(
    post,
    path = "/api/v1/librarian/memories",
    tag = "librarian",
    request_body = MemoryWriteRequestDto,
    responses(
        (status = 200, description = "The stored node", body = MemoryNodeDto),
        (status = 400, description = "Invalid input")
    )
)]
#[instrument(skip(state, body))]
pub async fn write_memory_handler(
    State(state): State<AppState>,
    Json(body): Json<MemoryWriteRequestDto>,
) -> Result<impl IntoResponse, RestError> {
    let key = body.key.trim().to_lowercase();
    if key.is_empty() {
        return Err(bad_request("The memory key is empty."));
    }
    let facts: Vec<(String, String)> = body
        .facts
        .iter()
        .filter_map(|fact| {
            let relation = fact.relation.trim().to_lowercase();
            let value = fact.value.trim();
            (!relation.is_empty() && !value.is_empty()).then(|| (relation, value.to_string()))
        })
        .collect();
    let node = state
        .stores
        .librarian
        .write(&key, body.title.trim(), body.body.trim(), &facts)
        .await
        .map_err(|err| {
            database_failed(
                err,
                "librarian write",
                "The daemon could not write the memory.",
            )
        })?;
    Ok(Json(node))
}

/// Reply of `GET /api/v1/librarian/lint`.
#[utoipa::path(get, path = "/api/v1/librarian/lint", tag = "librarian", responses((status = 200, description = "The memory linter report", body = LibrarianLintDto)))]
#[instrument(skip(state))]
pub async fn lint_librarian_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, RestError> {
    let report = state.stores.librarian.lint().await.map_err(|err| {
        database_failed(
            err,
            "librarian lint",
            "The daemon could not read the memory.",
        )
    })?;
    Ok(Json(report))
}

/// Reply of `DELETE /api/v1/librarian/facts/{id}`.
///
/// The fact is retired rather than deleted: it keeps its place in the
/// history and stops being true now. The reply carries the concept it
/// belongs to, so the editor reads the new state without a second call.
#[utoipa::path(
    delete,
    path = "/api/v1/librarian/facts/{id}",
    tag = "librarian",
    params(("id" = Uuid, Path, description = "Identifier of the fact")),
    responses(
        (status = 200, description = "The concept the fact belongs to", body = MemoryNodeDto),
        (status = 404, description = "The fact does not exist")
    )
)]
#[instrument(skip(state))]
pub async fn retire_fact_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let node = state
        .stores
        .librarian
        .retire_fact(id)
        .await
        .map_err(|err| {
            database_failed(
                err,
                "librarian fact retire",
                "The daemon could not change the memory.",
            )
        })?;
    match node {
        Some(node) => Ok(Json(node)),
        None => Err(not_found()),
    }
}

/// Reply of `POST /api/v1/librarian/memories/{id}/merge`.
///
/// The concept in the path moves into the concept the body names, so a
/// reader settles the two concepts the memory holds for one thing without
/// deleting either of them by hand. The reply is the concept that
/// absorbed the other, so the editor reads its new state without a second
/// call.
#[utoipa::path(
    post,
    path = "/api/v1/librarian/memories/{id}/merge",
    tag = "librarian",
    params(("id" = Uuid, Path, description = "Identifier of the concept that moves")),
    request_body = MemoryMergeRequestDto,
    responses(
        (status = 200, description = "The concept that absorbed the other", body = MemoryNodeDto),
        (status = 400, description = "The merge names one concept twice"),
        (status = 404, description = "One of the concepts does not exist")
    )
)]
#[instrument(skip(state, body))]
pub async fn merge_memory_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<MemoryMergeRequestDto>,
) -> Result<impl IntoResponse, RestError> {
    if id == body.into {
        return Err(bad_request("A concept cannot move into itself."));
    }
    let node = state
        .stores
        .librarian
        .merge_nodes(id, body.into)
        .await
        .map_err(|err| {
            database_failed(
                err,
                "librarian merge",
                "The daemon could not merge the memories.",
            )
        })?;
    match node {
        Some(node) => Ok(Json(node)),
        None => Err(not_found()),
    }
}

/// Reply of `DELETE /api/v1/librarian/memories/{id}`.
///
/// The whole concept goes, facts included. A reader who wants to keep the
/// history retires the one fact instead.
#[utoipa::path(
    delete,
    path = "/api/v1/librarian/memories/{id}",
    tag = "librarian",
    params(("id" = Uuid, Path, description = "Identifier of the concept")),
    responses(
        (status = 204, description = "The concept is deleted"),
        (status = 404, description = "The concept does not exist")
    )
)]
#[instrument(skip(state))]
pub async fn delete_memory_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let deleted = state
        .stores
        .librarian
        .delete_node(id)
        .await
        .map_err(|err| {
            database_failed(
                err,
                "librarian delete",
                "The daemon could not delete the memory.",
            )
        })?;
    if !deleted {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Search the memory and map a database failure to a reply.
async fn read_nodes(
    state: &AppState,
    text: String,
    limit: Option<u64>,
) -> Result<Vec<MemoryNodeDto>, RestError> {
    state
        .stores
        .librarian
        .query(&text, limit)
        .await
        .map_err(|err| {
            database_failed(
                err,
                "librarian query",
                "The daemon could not read the memory.",
            )
        })
}
