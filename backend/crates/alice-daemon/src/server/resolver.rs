//! Resolver handlers of the daemon.
//!
//! The settings page reads the state of the resolver here: which engine
//! reads a message, how many labels that engine has to read, and which
//! models and devices the built in GLiNER resolver can use.

use std::time::Duration;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::instrument;

use alice_core::config::{EmbedSource, ExtractEngine, ResolverBackend};
use alice_core::dto::{
    gliner_label_warning, GlinerStoreDto, LabelBudgetDto, LlamaStatusDto, LocalStoreDto,
    ResolverPreviewDto, ResolverPreviewRequestDto, ResolverStatusDto, RouterStatusDto,
    GLINER_HARD_LABEL_LIMIT, GLINER_SOFT_LABEL_LIMIT, LLAMA_INTENT_LIMIT,
};

/// The time the probe waits for the embedding server.
const EMBED_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// The text the probe embeds. One short word costs the server nothing.
const EMBED_PROBE_TEXT: &str = "alice";

use crate::chat;
use crate::handling;
use crate::resolver::device;
use crate::resolver::gliner::{
    active_device, available_devices, catalog, cuda_build, scoring, GlinerError, DEVICE_CPU,
};
use crate::resolver::local::{catalog as local_catalog, LocalError};
use crate::resolver::ResolveRequest;
use crate::server::reply::{bad_request, database_failed, error_reply, not_found, RestError};
use crate::server::rest::validate_text;
use crate::server::state::AppState;

/// Reply of `GET /api/v1/resolver`.
#[instrument(skip(state))]
pub async fn get_resolver_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, RestError> {
    Ok(Json(read_status(&state).await?))
}

/// Reply of `POST /api/v1/resolver/preview`.
///
/// The preview reads one message with the stored settings and runs
/// nothing, so the settings page can show the route of a sentence before
/// the user sends it. The reply carries the metadata of a stored turn, so
/// the route of a sentence and the route of a message the daemon really
/// handled read the same way.
#[instrument(skip(state, body))]
pub async fn preview_message_handler(
    State(state): State<AppState>,
    Json(body): Json<ResolverPreviewRequestDto>,
) -> Result<impl IntoResponse, RestError> {
    let text = validate_text(&body.text, state.config.queue.max_message_len)?;
    let deps = state.handling_deps();
    let outcome = handling::preview(
        &deps,
        &ResolveRequest {
            text: text.clone(),
            history: Vec::new(),
        },
    )
    .await;
    let meta = chat::meta_of(&outcome);
    let intent = outcome.intent.as_ref();

    Ok(Json(ResolverPreviewDto {
        text,
        matched: intent.is_some(),
        intent: intent.map(|intent| intent.name.clone()),
        confidence: intent.and_then(|intent| intent.confidence),
        reply: outcome.reply,
        meta,
    }))
}

/// Reply of `POST /api/v1/resolver/gliner/models/{id}/download`.
///
/// The download runs in the background, so the reply carries the state the
/// settings page polls while it runs.
#[instrument(skip(state))]
pub async fn download_gliner_model_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, RestError> {
    state
        .stores
        .gliner_store
        .start(&id)
        .map_err(|err| match err {
            GlinerError::UnknownModel { .. } => not_found(),
            GlinerError::DownloadRunning => {
                error_reply(StatusCode::CONFLICT, "A download already runs.")
            }
            other => bad_request(&other.to_string()),
        })?;
    Ok(Json(read_status(&state).await?))
}

/// Reply of `DELETE /api/v1/resolver/gliner/models/{id}`.
#[instrument(skip(state))]
pub async fn delete_gliner_model_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, RestError> {
    state
        .stores
        .gliner_store
        .remove(&id)
        .map_err(|err| match err {
            GlinerError::UnknownModel { .. } => not_found(),
            other => bad_request(&other.to_string()),
        })?;
    Ok(Json(read_status(&state).await?))
}

/// Reply of `POST /api/v1/resolver/local/models/{id}/download`.
///
/// The download runs in the background, so the reply carries the state the
/// settings page polls while it runs.
#[instrument(skip(state))]
pub async fn download_local_model_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, RestError> {
    state
        .stores
        .local_store
        .start(&id)
        .map_err(|err| match err {
            LocalError::UnknownModel { .. } => not_found(),
            LocalError::DownloadRunning => {
                error_reply(StatusCode::CONFLICT, "A download already runs.")
            }
            other => bad_request(&other.to_string()),
        })?;
    Ok(Json(read_status(&state).await?))
}

/// Reply of `DELETE /api/v1/resolver/local/models/{id}`.
#[instrument(skip(state))]
pub async fn delete_local_model_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, RestError> {
    state
        .stores
        .local_store
        .remove(&id)
        .map_err(|err| match err {
            LocalError::UnknownModel { .. } => not_found(),
            other => bad_request(&other.to_string()),
        })?;
    Ok(Json(read_status(&state).await?))
}

/// Read the state of the resolver.
pub async fn read_status(state: &AppState) -> Result<ResolverStatusDto, RestError> {
    let settings = state.stores.settings.get_settings().await.map_err(|err| {
        database_failed(
            err,
            "settings read",
            "The daemon could not read the settings.",
        )
    })?;
    let intents = state.stores.intents.list_intents().await.map_err(|err| {
        database_failed(err, "intent list", "The daemon could not list the intents.")
    })?;

    let store = &state.stores.gliner_store;
    let spec = catalog::find(&settings.gliner_model).unwrap_or_else(catalog::default_spec);
    let labels = scoring::label_count(&intents);
    // The label budget belongs to the reader of the labels. The router
    // reads the labels only when its extraction stage asks the built in
    // model for a span, so a router that reads the lists alone is not
    // held back by a label count.
    let warning = match settings.resolver_backend {
        ResolverBackend::Router => match settings.router.extract {
            ExtractEngine::Spans => gliner_label_warning(labels),
            ExtractEngine::Lists | ExtractEngine::Generative => None,
        },
        ResolverBackend::Llama | ResolverBackend::Gliner | ResolverBackend::Hybrid => {
            gliner_label_warning(labels)
        }
    };
    let intent_limit = match settings.resolver_backend {
        // The router ranks the whole catalog and lets a model read only
        // the short list of one turn, so the catalog has no ceiling.
        ResolverBackend::Router | ResolverBackend::Gliner => None,
        ResolverBackend::Llama | ResolverBackend::Hybrid => Some(LLAMA_INTENT_LIMIT),
    };
    let (embeddings_reachable, embeddings_detail) = probe_embeddings(state, &settings).await;
    let local_store = &state.stores.local_store;
    let active_local_device = device::active_device(settings.router.local_device)
        .unwrap_or(DEVICE_CPU)
        .to_string();

    Ok(ResolverStatusDto {
        backend: settings.resolver_backend.as_str().to_string(),
        llama: LlamaStatusDto {
            base_url: settings.resolver_base_url.clone(),
            model: settings.resolver_model.clone(),
            intent_limit: LLAMA_INTENT_LIMIT,
        },
        gliner: GlinerStoreDto {
            model: spec.id.to_string(),
            device: settings.gliner_device.as_str().to_string(),
            active_device: active_device(settings.gliner_device)
                .unwrap_or(DEVICE_CPU)
                .to_string(),
            installed: store.is_installed(spec),
            threshold: settings.gliner_threshold,
            models_dir: store.root().display().to_string(),
            devices: available_devices()
                .into_iter()
                .map(str::to_string)
                .collect(),
            cuda_build: cuda_build(),
            models: store.list(),
            download: store.download_state().map(|state| state.to_dto()),
        },
        budget: LabelBudgetDto {
            labels,
            soft_limit: GLINER_SOFT_LABEL_LIMIT,
            hard_limit: GLINER_HARD_LABEL_LIMIT,
            intents: intents.len(),
            intent_limit,
            over_soft: labels.saturating_sub(GLINER_SOFT_LABEL_LIMIT),
            over_hard: labels.saturating_sub(GLINER_HARD_LABEL_LIMIT),
            warning: warning.map(|(message, _)| message),
        },
        router: RouterStatusDto {
            fast_path: settings.router.fast_path,
            retrieve: settings.router.retrieve.as_str().to_string(),
            decide: settings.router.decide.as_str().to_string(),
            extract: settings.router.extract.as_str().to_string(),
            top_k: settings.router.top_k,
            floor: settings.router.floor,
            margin: settings.router.margin,
            lexical_weight: settings.router.lexical_weight,
            dense_weight: settings.router.dense_weight,
            embed_model: settings.router.embed_model.clone(),
            embed_source: settings.router.embed_source.as_str().to_string(),
            embed_local_model: settings.router.embed_local_model.clone(),
            rerank_model: settings.router.rerank_model.clone(),
            local_device: settings.router.local_device.as_str().to_string(),
            active_local_device,
            local: LocalStoreDto {
                models_dir: local_store.root().display().to_string(),
                models: local_store.list(),
                download: local_store.download_state().map(|state| state.to_dto()),
            },
            phrase_gate: settings.router.phrase_gate,
            list_match: settings.router.list_match.as_str().to_string(),
            list_floor: settings.router.list_floor,
            embeddings_reachable,
            embeddings_detail,
        },
    })
}

/// Report whether the place the vectors come from answers right now.
///
/// The router reads embeddings only when the settings ask for them, so a
/// router that reads the words alone reports that reason rather than a
/// failure of a place it never calls. A router that reads a built in model
/// reports whether that model is on disk, because a file cannot answer a
/// port.
async fn probe_embeddings(
    state: &AppState,
    settings: &crate::settings::SettingsValues,
) -> (bool, Option<String>) {
    let router = &settings.router;
    if !(router.retrieve.uses_embeddings() || router.list_match.uses_embeddings()) {
        return (
            false,
            Some("The router reads the words of the catalog alone.".to_string()),
        );
    }
    if matches!(router.embed_source, EmbedSource::Local) {
        return match local_catalog::find_role(
            &router.embed_local_model,
            local_catalog::Role::Embedding,
        ) {
            Some(spec) if state.stores.local_store.is_installed(spec) => (true, None),
            Some(spec) => (
                false,
                Some(format!(
                    "The built in model `{}` is not on disk. Install it here.",
                    spec.id
                )),
            ),
            None => (
                false,
                Some(format!(
                    "`{}` is not a built in embedding model this daemon knows.",
                    router.embed_local_model
                )),
            ),
        };
    }
    let input = vec![EMBED_PROBE_TEXT.to_string()];
    match state
        .stores
        .resolver
        .client()
        .embed(
            &settings.resolver_base_url,
            &settings.router.embed_model,
            &input,
            EMBED_PROBE_TIMEOUT,
        )
        .await
    {
        Ok(vectors) if !vectors.is_empty() => (true, None),
        Ok(_) => (
            false,
            Some("The embedding server answered with no vector.".to_string()),
        ),
        Err(err) => {
            tracing::debug!(error = %err, "the embedding server does not answer");
            (false, Some(err.to_string()))
        }
    }
}
