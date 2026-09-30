//! OpenAPI specification of the daemon.
//! This module generates the contract from the DTOs and the handlers.

use axum::{http::header, response::IntoResponse, Json};
use utoipa::OpenApi;

/// OpenAPI specification owned by the backend.
///
/// The frontend copies the DTOs from the generated file at
/// `backend/config/openapi.yaml`. The spec is generated at build time or
/// served live at `GET /api/docs/openapi.json`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Alice Voice Backend API",
        version = "0.1.0",
        description = "REST and socket contract owned by the backend. Frontend copies DTOs from this file."
    ),
    servers(
        (url = "http://127.0.0.1:8787", description = "Local daemon")
    ),
    paths(
        crate::server::rest::health_handler,
        crate::server::rest::health_alias_handler,
        crate::server::rest::status_handler,
        crate::server::rest::chat_handler,
        crate::server::scripts::get_script_handler,
        crate::server::scripts::approve_script_handler,
        crate::server::scripts::deny_script_handler,
        crate::server::conversations::list_conversations_handler,
        crate::server::conversations::get_conversation_handler,
        crate::server::intents::list_intents_handler,
        crate::server::intents::create_intent_handler,
        crate::server::intents::update_intent_handler,
        crate::server::intents::delete_intent_handler,
        crate::server::intents::preview_script_handler,
        crate::server::intents::create_examples_handler,
        crate::server::settings::get_settings_handler,
        crate::server::settings::put_settings_handler,
        crate::server::librarian::get_librarian_handler,
        crate::server::librarian::list_memories_handler,
        crate::server::librarian::query_memory_handler,
        crate::server::librarian::write_memory_handler,
        crate::server::librarian::retire_fact_handler,
        crate::server::librarian::delete_memory_handler,
        crate::server::librarian::merge_memory_handler,
        crate::server::librarian::lint_librarian_handler,
        crate::server::dependencies::get_dependencies_handler,
        crate::server::resolver::get_resolver_handler,
        crate::server::resolver::preview_message_handler,
        crate::server::resolver::download_gliner_model_handler,
        crate::server::resolver::delete_gliner_model_handler,
        crate::server::resolver::download_local_model_handler,
        crate::server::resolver::delete_local_model_handler,
        crate::server::socket::events_handler,
        crate::server::system::get_system_handler,
        crate::server::system::post_system_preset_handler,
    ),
    components(schemas(
        alice_core::dto::DaemonStateDto,
        alice_core::dto::StopReasonDto,
        alice_core::dto::OutputStreamDto,
        alice_core::dto::ChatRoleDto,
        alice_core::dto::EntityKindDto,
        alice_core::dto::StatusDto,
        alice_core::dto::HealthDto,
        alice_core::dto::ChatMessageDto,
        alice_core::dto::MessageMetaDto,
        alice_core::dto::MessageRouteDto,
        alice_core::dto::MessageRouteStepDto,
        alice_core::dto::MessageEntityDto,
        alice_core::dto::MessageCandidateDto,
        alice_core::dto::ConversationDto,
        alice_core::dto::ConversationListDto,
        alice_core::dto::ConversationDetailDto,
        alice_core::dto::ConversationQuery,
        alice_core::dto::ChatRequestDto,
        alice_core::dto::ChatReplyDto,
        alice_core::dto::ScriptDto,
        alice_core::dto::ResolverPreviewRequestDto,
        alice_core::dto::ResolverPreviewDto,
        alice_core::dto::IntentEntityDto,
        alice_core::dto::IntentDto,
        alice_core::dto::IntentListDto,
        alice_core::dto::IntentEntityWriteDto,
        alice_core::dto::IntentWriteDto,
        alice_core::dto::ScriptPreviewWriteDto,
        alice_core::dto::ScriptPreviewDto,
        alice_core::dto::SettingsDto,
        alice_core::dto::SettingsUpdateDto,
        alice_core::dto::LibrarianStatusDto,
        alice_core::dto::MemoryFactDto,
        alice_core::dto::MemoryNodeDto,
        alice_core::dto::MemoryQueryRequestDto,
        alice_core::dto::MemoryQueryDto,
        alice_core::dto::MemoryFactWriteDto,
        alice_core::dto::MemoryWriteRequestDto,
        alice_core::dto::LibrarianLintDto,
        alice_core::dto::MemoryMergeRequestDto,
        alice_core::dto::DependencyDto,
        alice_core::dto::LlamaDependencyDto,
        alice_core::dto::DependenciesDto,
        alice_core::dto::ResolverStatusDto,
        alice_core::dto::RouterStatusDto,
        alice_core::dto::LlamaStatusDto,
        alice_core::dto::GlinerStoreDto,
        alice_core::dto::GlinerModelDto,
        alice_core::dto::GlinerDownloadDto,
        alice_core::dto::LocalModelDto,
        alice_core::dto::LocalDownloadDto,
        alice_core::dto::LocalStoreDto,
        alice_core::dto::LabelBudgetDto,
        alice_core::dto::SystemOsDto,
        alice_core::dto::SystemCpuCoreDto,
        alice_core::dto::SystemCpuDto,
        alice_core::dto::SystemMemoryDto,
        alice_core::dto::SystemDiskDto,
        alice_core::dto::SystemDevicesDto,
        alice_core::dto::SystemResolverDto,
        alice_core::dto::RouterPresetDto,
        alice_core::dto::SystemRecommendationDto,
        alice_core::dto::SystemProfileDto,
        alice_core::dto::SystemPresetRequestDto,
        alice_core::dto::SystemPresetDto,
        alice_core::dto::SystemEventDto,
        alice_core::dto::SystemEventPayloadDto,
    ))
)]
pub struct ApiDoc;

/// Return the generated OpenAPI specification.
pub fn spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Serve the specification as JSON at `GET /api/docs/openapi.json`.
pub async fn openapi_json_handler() -> impl IntoResponse {
    Json(spec())
}

/// Serve the specification as YAML at `GET /api/docs/openapi.yaml`.
pub async fn openapi_yaml_handler() -> impl IntoResponse {
    let yaml = serde_yaml::to_string(&spec()).unwrap_or_else(|_| String::new());
    ([(header::CONTENT_TYPE, "text/yaml; charset=utf-8")], yaml)
}
