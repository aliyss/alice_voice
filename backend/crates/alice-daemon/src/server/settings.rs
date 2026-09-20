//! Settings handlers of the daemon.

use std::path::PathBuf;

use axum::{extract::State, response::IntoResponse, Json};
use tracing::instrument;

use alice_core::config::{
    DecideEngine, EmbedSource, ExtractEngine, ListMatch, LocalDevice, ResolverBackend,
    RetrieveEngine, RouterConfig,
};
use alice_core::dto::{
    SettingsDto, SettingsUpdateDto, PREVIEW_SENTENCE_LIMIT, PREVIEW_SENTENCE_MAX,
};

use crate::resolver::gliner::catalog;
use crate::resolver::local::catalog as local_catalog;
use crate::server::reply::{bad_request, database_failed, RestError};
use crate::server::state::AppState;
use crate::settings::SettingsValues;

/// The start of every accepted resolver URL.
const URL_SCHEMES: [&str; 2] = ["http://", "https://"];

/// Reply of `GET /api/v1/settings`.
#[instrument(skip(state))]
pub async fn get_settings_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, RestError> {
    let values = read_settings(&state).await?;
    Ok(Json(to_dto(values)))
}

/// Reply of `PUT /api/v1/settings`.
///
/// A field the caller does not send keeps its stored value, so a caller
/// may change one setting without sending the others.
#[instrument(skip(state, body))]
pub async fn put_settings_handler(
    State(state): State<AppState>,
    Json(body): Json<SettingsUpdateDto>,
) -> Result<impl IntoResponse, RestError> {
    // 1. Validate. The router stages are read from the stored set first,
    //    because the write carries one field at a time and the whole set
    //    is written back.
    let stored_router = if body.changes_the_router() {
        Some(state.stores.settings.get_router().await.map_err(|err| {
            database_failed(
                err,
                "settings read",
                "The daemon could not read the settings.",
            )
        })?)
    } else {
        None
    };
    let router = match stored_router {
        Some(stored) => Some(validate_router(&body, stored)?),
        None => None,
    };
    let base_url = match body.resolver_base_url {
        Some(url) => Some(validate_base_url(&url)?),
        None => None,
    };
    let model = match body.resolver_model {
        Some(model) => Some(require_text(&model, "The resolver model is empty.")?),
        None => None,
    };
    let backend = match body.resolver_backend {
        Some(backend) => Some(validate_backend(&backend)?),
        None => None,
    };
    let gliner_model = match body.gliner_model {
        Some(model) => Some(validate_gliner_model(&model)?),
        None => None,
    };
    let gliner_device = match body.gliner_device {
        Some(device) => Some(validate_device(&device)?),
        None => None,
    };
    let gliner_threshold = match body.gliner_threshold {
        Some(threshold) => Some(validate_threshold(threshold)?),
        None => None,
    };
    let preview_sentences = match &body.preview_sentences {
        Some(sentences) => Some(validate_preview_sentences(sentences)?),
        None => None,
    };

    // 2. Write what the caller sent
    let settings = &state.stores.settings;
    if let Some(enabled) = body.queue_enabled {
        settings.set_queue_enabled(enabled).await.map_err(|err| {
            database_failed(
                err,
                "settings write",
                "The daemon could not save the settings.",
            )
        })?;
    }
    if let Some(url) = base_url {
        settings.set_resolver_base_url(&url).await.map_err(|err| {
            database_failed(
                err,
                "settings write",
                "The daemon could not save the settings.",
            )
        })?;
    }
    if let Some(model) = model {
        settings.set_resolver_model(&model).await.map_err(|err| {
            database_failed(
                err,
                "settings write",
                "The daemon could not save the settings.",
            )
        })?;
    }
    if let Some(backend) = backend {
        settings
            .set_resolver_backend(backend)
            .await
            .map_err(|err| {
                database_failed(
                    err,
                    "settings write",
                    "The daemon could not save the settings.",
                )
            })?;
    }
    if let Some(model) = gliner_model {
        settings.set_gliner_model(&model).await.map_err(|err| {
            database_failed(
                err,
                "settings write",
                "The daemon could not save the settings.",
            )
        })?;
    }
    if let Some(device) = gliner_device {
        settings.set_gliner_device(device).await.map_err(|err| {
            database_failed(
                err,
                "settings write",
                "The daemon could not save the settings.",
            )
        })?;
    }
    if let Some(threshold) = gliner_threshold {
        settings
            .set_gliner_threshold(threshold)
            .await
            .map_err(|err| {
                database_failed(
                    err,
                    "settings write",
                    "The daemon could not save the settings.",
                )
            })?;
    }
    if let Some(router) = router {
        settings.set_router(&router).await.map_err(|err| {
            database_failed(
                err,
                "settings write",
                "The daemon could not save the settings.",
            )
        })?;
    }
    if let Some(sentences) = preview_sentences {
        settings
            .set_preview_sentences(&sentences)
            .await
            .map_err(|err| {
                database_failed(
                    err,
                    "settings write",
                    "The daemon could not save the settings.",
                )
            })?;
    }

    // 3. Return the stored settings
    let values = read_settings(&state).await?;
    Ok(Json(to_dto(values)))
}

/// Read the stored settings.
async fn read_settings(state: &AppState) -> Result<SettingsValues, RestError> {
    state.stores.settings.get_settings().await.map_err(|err| {
        database_failed(
            err,
            "settings read",
            "The daemon could not read the settings.",
        )
    })
}

/// Map the settings to the reply of the endpoint.
fn to_dto(values: SettingsValues) -> SettingsDto {
    SettingsDto {
        queue_enabled: values.queue_enabled,
        resolver_backend: values.resolver_backend.as_str().to_string(),
        resolver_base_url: values.resolver_base_url,
        resolver_model: values.resolver_model,
        gliner_model: values.gliner_model,
        gliner_device: values.gliner_device.as_str().to_string(),
        gliner_threshold: values.gliner_threshold,
        router_fast_path: values.router.fast_path,
        router_retrieve: values.router.retrieve.as_str().to_string(),
        router_decide: values.router.decide.as_str().to_string(),
        router_extract: values.router.extract.as_str().to_string(),
        router_top_k: values.router.top_k,
        router_floor: values.router.floor,
        router_margin: values.router.margin,
        router_lexical_weight: values.router.lexical_weight,
        router_dense_weight: values.router.dense_weight,
        router_embed_model: values.router.embed_model,
        router_models_dir: values.router.models_dir.display().to_string(),
        router_embed_source: values.router.embed_source.as_str().to_string(),
        router_embed_local_model: values.router.embed_local_model,
        router_rerank_model: values.router.rerank_model,
        router_local_device: values.router.local_device.as_str().to_string(),
        router_phrase_gate: values.router.phrase_gate,
        router_list_match: values.router.list_match.as_str().to_string(),
        router_list_floor: values.router.list_floor,
        preview_sentences: values.preview_sentences,
    }
}

/// Read the sentences of a settings write.
///
/// A sentence is trimmed, and an empty one is dropped, because a blank
/// line in a list of tests is a line the user did not mean to write. The
/// list keeps the order of the user and holds every sentence once.
fn validate_preview_sentences(sentences: &[String]) -> Result<Vec<String>, RestError> {
    if sentences.len() > PREVIEW_SENTENCE_LIMIT {
        return Err(bad_request(&format!(
            "Keep at most {PREVIEW_SENTENCE_LIMIT} sentences."
        )));
    }
    let mut kept: Vec<String> = Vec::with_capacity(sentences.len());
    for sentence in sentences {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.len() > PREVIEW_SENTENCE_MAX {
            return Err(bad_request(&format!(
                "A sentence holds at most {PREVIEW_SENTENCE_MAX} characters."
            )));
        }
        if !kept.iter().any(|held| held == trimmed) {
            kept.push(trimmed.to_string());
        }
    }
    Ok(kept)
}

/// Read the router stages of a settings write.
///
/// A field the caller does not send keeps the stored value, so the page
/// may change one stage without sending the others.
fn validate_router(
    body: &SettingsUpdateDto,
    mut router: RouterConfig,
) -> Result<RouterConfig, RestError> {
    if let Some(value) = body.router_fast_path {
        router.fast_path = value;
    }
    if let Some(value) = &body.router_retrieve {
        router.retrieve = read_retrieve(value)?;
    }
    if let Some(value) = &body.router_decide {
        router.decide = read_decide(value)?;
    }
    if let Some(value) = &body.router_extract {
        router.extract = read_extract(value)?;
    }
    if let Some(value) = body.router_top_k {
        if !(1..=64).contains(&value) {
            return Err(bad_request(
                "The short list must hold between 1 and 64 intents.",
            ));
        }
        router.top_k = value;
    }
    if let Some(value) = body.router_floor {
        router.floor = read_unit(value, "The floor")?;
    }
    if let Some(value) = body.router_margin {
        router.margin = read_unit(value, "The margin")?;
    }
    if let Some(value) = body.router_lexical_weight {
        router.lexical_weight = read_weight(value, "The weight of the words")?;
    }
    if let Some(value) = body.router_dense_weight {
        router.dense_weight = read_weight(value, "The weight of the embeddings")?;
    }
    if let Some(value) = &body.router_embed_model {
        router.embed_model = require_text(value, "The embedding model is empty.")?;
    }
    if let Some(value) = &body.router_models_dir {
        router.models_dir = PathBuf::from(require_text(value, "The model directory is empty.")?);
    }
    if let Some(value) = &body.router_embed_source {
        router.embed_source = read_embed_source(value)?;
    }
    if let Some(value) = &body.router_embed_local_model {
        router.embed_local_model = validate_local_model(value, local_catalog::Role::Embedding)?;
    }
    if let Some(value) = &body.router_rerank_model {
        router.rerank_model = validate_local_model(value, local_catalog::Role::Reranker)?;
    }
    if let Some(value) = &body.router_local_device {
        router.local_device = validate_device(value)?;
    }
    if let Some(value) = body.router_phrase_gate {
        router.phrase_gate = value;
    }
    if let Some(value) = &body.router_list_match {
        router.list_match = read_list_match(value)?;
    }
    if let Some(value) = body.router_list_floor {
        router.list_floor = read_unit(value, "The list floor")?;
    }

    // A configuration the daemon cannot run is a configuration it must
    // not store, so the whole set is held against the same rules the
    // configuration file is held against.
    if router.lexical_weight == 0.0 && router.dense_weight == 0.0 {
        return Err(bad_request(
            "The words and the embeddings cannot both weigh nothing.",
        ));
    }
    if router.retrieve.uses_embeddings() && router.dense_weight == 0.0 {
        return Err(bad_request(
            "The embeddings weigh nothing while the retrieval reads them.",
        ));
    }
    Ok(router)
}

/// Read the place the vectors come from out of a settings write.
fn read_embed_source(value: &str) -> Result<EmbedSource, RestError> {
    match value.trim().to_lowercase().as_str() {
        "server" => Ok(EmbedSource::Server),
        "local" => Ok(EmbedSource::Local),
        _ => Err(bad_request(
            "The embeddings must come from the model server or from a built in model.",
        )),
    }
}

/// Read one engine of the retrieval stage out of a settings write.
fn read_retrieve(value: &str) -> Result<RetrieveEngine, RestError> {
    match value.trim().to_lowercase().as_str() {
        "lexical" => Ok(RetrieveEngine::Lexical),
        "dense" => Ok(RetrieveEngine::Dense),
        "hybrid" => Ok(RetrieveEngine::Hybrid),
        _ => Err(bad_request(
            "The retrieval stage must read the words, the embeddings, or both.",
        )),
    }
}

/// Read one engine of the decision stage out of a settings write.
fn read_decide(value: &str) -> Result<DecideEngine, RestError> {
    match value.trim().to_lowercase().as_str() {
        "score" => Ok(DecideEngine::Score),
        "rerank" => Ok(DecideEngine::Rerank),
        "generative" => Ok(DecideEngine::Generative),
        _ => Err(bad_request(
            "The decision stage must decide by the scores, by the built in reranker, or by a model.",
        )),
    }
}

/// Read one engine of the extraction stage out of a settings write.
fn read_extract(value: &str) -> Result<ExtractEngine, RestError> {
    match value.trim().to_lowercase().as_str() {
        "lists" => Ok(ExtractEngine::Lists),
        "spans" => Ok(ExtractEngine::Spans),
        "generative" => Ok(ExtractEngine::Generative),
        _ => Err(bad_request(
            "The extraction stage must read the lists, the spans, or a model.",
        )),
    }
}

/// Read the rule set of a list out of a settings write.
fn read_list_match(value: &str) -> Result<ListMatch, RestError> {
    match value.trim().to_lowercase().as_str() {
        "lexical" => Ok(ListMatch::Lexical),
        "dense" => Ok(ListMatch::Dense),
        "both" => Ok(ListMatch::Both),
        _ => Err(bad_request(
            "The list matching must read the rules, the embeddings, or both.",
        )),
    }
}

/// Read one probability out of a settings write.
fn read_unit(value: f32, name: &str) -> Result<f32, RestError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(bad_request(&format!("{name} must be between 0 and 1.")));
    }
    Ok(value)
}

/// Read one weight out of a settings write.
fn read_weight(value: f32, name: &str) -> Result<f32, RestError> {
    if !value.is_finite() || value < 0.0 {
        return Err(bad_request(&format!("{name} must be zero or more.")));
    }
    Ok(value)
}

/// Read the backend out of a settings write.
fn validate_backend(value: &str) -> Result<ResolverBackend, RestError> {
    match value.trim().to_lowercase().as_str() {
        "llama" => Ok(ResolverBackend::Llama),
        "gliner" => Ok(ResolverBackend::Gliner),
        "hybrid" => Ok(ResolverBackend::Hybrid),
        "router" => Ok(ResolverBackend::Router),
        _ => Err(bad_request(
            "The resolver backend must be router, hybrid, llama, or gliner.",
        )),
    }
}

/// Read the device out of a settings write.
fn validate_device(value: &str) -> Result<LocalDevice, RestError> {
    match value.trim().to_lowercase().as_str() {
        "auto" => Ok(LocalDevice::Auto),
        "cpu" => Ok(LocalDevice::Cpu),
        "cuda" => Ok(LocalDevice::Cuda),
        _ => Err(bad_request("The GLiNER device must be auto, cpu, or cuda.")),
    }
}

/// Read the identifier of a built in model out of a settings write.
///
/// The daemon downloads a model by its identifier, so the settings page
/// may only store one the catalog knows.
fn validate_gliner_model(value: &str) -> Result<String, RestError> {
    let id = require_text(value, "The GLiNER model is empty.")?;
    match catalog::find(&id) {
        Some(spec) => Ok(spec.id.to_string()),
        None => Err(bad_request(
            "That GLiNER model is not one the daemon can download.",
        )),
    }
}

/// Read the identifier of a built in model of the router out of a
/// settings write.
///
/// The daemon downloads a model by its identifier, so the settings page
/// may only store one the catalog knows for that role.
fn validate_local_model(value: &str, role: local_catalog::Role) -> Result<String, RestError> {
    let id = require_text(value, "The built in model is empty.")?;
    match local_catalog::find_role(&id, role) {
        Some(spec) => Ok(spec.id.to_string()),
        None => Err(bad_request(&format!(
            "That model reads no {}.",
            role.label()
        ))),
    }
}

/// Read the threshold out of a settings write.
fn validate_threshold(value: f32) -> Result<f32, RestError> {
    if !(0.0..=1.0).contains(&value) {
        return Err(bad_request("The GLiNER threshold must be between 0 and 1."));
    }
    Ok(value)
}

/// Check that a text carries a value and return it trimmed.
fn require_text(text: &str, message: &str) -> Result<String, RestError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(bad_request(message));
    }
    Ok(trimmed.to_string())
}

/// Check that a value is an HTTP address and return it trimmed.
fn validate_base_url(url: &str) -> Result<String, RestError> {
    let trimmed = require_text(url, "The resolver URL is empty.")?;
    if !URL_SCHEMES.iter().any(|scheme| trimmed.starts_with(scheme)) {
        return Err(bad_request(
            "The resolver URL must start with http:// or https://.",
        ));
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_base_url_keeps_a_http_address() {
        let url = validate_base_url("  http://127.0.0.1:8012/v1  ").expect("the url is valid");
        assert_eq!(url, "http://127.0.0.1:8012/v1");
    }

    #[test]
    fn validate_base_url_rejects_a_bare_host() {
        assert!(validate_base_url("127.0.0.1:8012").is_err());
    }

    #[test]
    fn validate_base_url_rejects_an_empty_value() {
        assert!(validate_base_url("   ").is_err());
    }

    #[test]
    fn validate_backend_reads_the_three_engines() {
        assert_eq!(
            validate_backend(" llama ").ok(),
            Some(ResolverBackend::Llama)
        );
        assert_eq!(
            validate_backend("GLINER").ok(),
            Some(ResolverBackend::Gliner)
        );
        assert_eq!(
            validate_backend("hybrid").ok(),
            Some(ResolverBackend::Hybrid)
        );
        assert!(validate_backend("openai").is_err());
    }

    #[test]
    fn validate_device_reads_the_three_devices() {
        assert_eq!(validate_device(" auto ").ok(), Some(LocalDevice::Auto));
        assert_eq!(validate_device("cpu").ok(), Some(LocalDevice::Cpu));
        assert_eq!(validate_device("CUDA").ok(), Some(LocalDevice::Cuda));
        assert!(validate_device("tpu").is_err());
    }

    #[test]
    fn validate_gliner_model_keeps_a_model_of_the_catalog() {
        assert_eq!(
            validate_gliner_model("gliner_small-v2.1").ok(),
            Some("gliner_small-v2.1".to_string())
        );
        assert!(validate_gliner_model("gliner_nope").is_err());
        assert!(validate_gliner_model("  ").is_err());
    }

    #[test]
    fn validate_preview_sentences_trims_and_drops_the_empty_ones() {
        let sentences = validate_preview_sentences(&[
            "  open firefox  ".to_string(),
            "   ".to_string(),
            "what is the weather".to_string(),
        ])
        .expect("the sentences are valid");

        assert_eq!(
            sentences,
            vec![
                "open firefox".to_string(),
                "what is the weather".to_string()
            ]
        );
    }

    #[test]
    fn validate_preview_sentences_keeps_one_of_a_sentence() {
        let sentences =
            validate_preview_sentences(&["open firefox".to_string(), "open firefox".to_string()])
                .expect("the sentences are valid");

        assert_eq!(sentences, vec!["open firefox".to_string()]);
    }

    #[test]
    fn validate_preview_sentences_refuses_a_list_without_room() {
        let many: Vec<String> = (0..=PREVIEW_SENTENCE_LIMIT)
            .map(|index| format!("sentence {index}"))
            .collect();

        assert!(validate_preview_sentences(&many).is_err());
    }

    #[test]
    fn validate_preview_sentences_refuses_a_sentence_that_is_too_long() {
        let long = vec!["a".repeat(PREVIEW_SENTENCE_MAX + 1)];

        assert!(validate_preview_sentences(&long).is_err());
    }

    #[test]
    fn validate_threshold_stays_between_zero_and_one() {
        assert_eq!(validate_threshold(0.3).ok(), Some(0.3));
        assert!(validate_threshold(-0.1).is_err());
        assert!(validate_threshold(1.2).is_err());
    }
}
