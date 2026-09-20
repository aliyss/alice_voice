//! Failure replies of the API server.
//! This module builds the replies that more than one handler uses.

use axum::{http::StatusCode, Json};

/// The failure reply of a REST handler.
pub type RestError = (StatusCode, Json<serde_json::Value>);

/// Build a failure reply with a message for the user.
pub fn error_reply(status: StatusCode, message: &str) -> RestError {
    (status, Json(serde_json::json!({ "error": message })))
}

/// Build a bad request reply.
pub fn bad_request(message: &str) -> RestError {
    error_reply(StatusCode::BAD_REQUEST, message)
}

/// Build a not found reply.
pub fn not_found() -> RestError {
    error_reply(StatusCode::NOT_FOUND, "The resource does not exist.")
}

/// Log a database failure and build the reply for it.
pub fn database_failed(error: sea_orm::DbErr, context: &str, message: &str) -> RestError {
    tracing::error!(error = %error, context, "database call failed");
    error_reply(StatusCode::INTERNAL_SERVER_ERROR, message)
}
