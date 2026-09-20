//! Route table of the API server.
//! This module builds the Axum router.

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::server::conversations::{get_conversation_handler, list_conversations_handler};
use crate::server::dependencies::get_dependencies_handler;
use crate::server::intents::{
    create_examples_handler, create_intent_handler, delete_intent_handler, list_intents_handler,
    preview_script_handler, update_intent_handler,
};
use crate::server::resolver::{
    delete_gliner_model_handler, delete_local_model_handler, download_gliner_model_handler,
    download_local_model_handler, get_resolver_handler, preview_message_handler,
};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::server::openapi::openapi_yaml_handler;
use crate::server::rest::{chat_handler, health_alias_handler, health_handler, status_handler};
use crate::server::settings::{get_settings_handler, put_settings_handler};
use crate::server::socket::events_handler;
use crate::server::state::AppState;

/// Build the API router with the shared state.
pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let swagger = SwaggerUi::new("/docs").url(
        "/api/docs/openapi.json",
        crate::server::openapi::ApiDoc::openapi(),
    );

    Router::new()
        .route("/api/health", get(health_handler))
        .route("/health", get(health_alias_handler))
        .route("/api/docs/openapi.yaml", get(openapi_yaml_handler))
        .merge(swagger)
        .route("/api/v1/status", get(status_handler))
        .route("/api/v1/conversations", get(list_conversations_handler))
        .route("/api/v1/conversations/{id}", get(get_conversation_handler))
        .route(
            "/api/v1/intents",
            get(list_intents_handler).post(create_intent_handler),
        )
        .route(
            "/api/v1/intents/{id}",
            put(update_intent_handler).delete(delete_intent_handler),
        )
        .route(
            "/api/v1/intents/script/preview",
            post(preview_script_handler),
        )
        .route("/api/v1/intents/examples", post(create_examples_handler))
        .route("/api/v1/chat", post(chat_handler))
        .route("/api/v1/events", get(events_handler))
        .route("/api/v1/dependencies", get(get_dependencies_handler))
        .route("/api/v1/resolver", get(get_resolver_handler))
        .route("/api/v1/resolver/preview", post(preview_message_handler))
        .route(
            "/api/v1/resolver/gliner/models/{id}/download",
            post(download_gliner_model_handler),
        )
        .route(
            "/api/v1/resolver/gliner/models/{id}",
            delete(delete_gliner_model_handler),
        )
        .route(
            "/api/v1/resolver/local/models/{id}/download",
            post(download_local_model_handler),
        )
        .route(
            "/api/v1/resolver/local/models/{id}",
            delete(delete_local_model_handler),
        )
        .route(
            "/api/v1/settings",
            get(get_settings_handler).put(put_settings_handler),
        )
        .layer(cors)
        .with_state(state)
}
