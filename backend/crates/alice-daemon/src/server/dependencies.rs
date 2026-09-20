//! Dependency handlers of the daemon.
//!
//! A setting belongs to a place that stores it and to a service that makes
//! it work. The queue is a database setting, the llama.cpp model is a
//! database setting and a server setting, and the built in GLiNER model
//! needs a model on disk.
//!
//! This module reports which of those answer, so the settings page keeps a
//! control usable only when every place it needs answers. The reply needs
//! no database, so the page can tell a database that is down from a daemon
//! that is down.

use std::time::Duration;

use axum::{extract::State, response::IntoResponse, Json};
use sea_orm::DatabaseConnection;
use tracing::instrument;

use alice_core::config::{CoreConfig, LocalDevice};
use alice_core::dto::{DependenciesDto, DependencyDto, LlamaDependencyDto, SettingsDto};

use crate::resolver::gliner::{active_device, catalog, GlinerStore};
use crate::server::state::AppState;

/// The time the probe waits for the model server.
const LLAMA_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// Reply of `GET /api/v1/dependencies`.
#[utoipa::path(get, path = "/api/v1/dependencies", tag = "system", responses((status = 200, description = "State of the dependencies", body = DependenciesDto)))]
#[instrument(skip(state))]
pub async fn get_dependencies_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(read_dependencies(&state).await)
}

/// Read the state of every dependency of the settings surface.
pub async fn read_dependencies(state: &AppState) -> DependenciesDto {
    // 1. The database. Every setting lives there, so its answer decides
    //    whether the settings page may store a value at all.
    let database = probe_database(&state.stores.db).await;

    // 2. The values of the settings. They come from the database when it
    //    answers, and from the configuration when it does not, so the
    //    probes below still know which addresses to ask.
    let settings = match database.reachable {
        true => state
            .stores
            .settings
            .get_settings()
            .await
            .map(|values| to_dto(&values))
            .unwrap_or_else(|_| configured_settings(&state.config)),
        false => configured_settings(&state.config),
    };

    // 3. The llama.cpp server, and the models it offers.
    let llama = probe_llama(state, &settings).await;

    // 4. The built in GLiNER resolver: a model on disk and a usable device.
    let gliner = probe_gliner(&state.stores.gliner_store, &settings);

    DependenciesDto {
        configured: configured_settings(&state.config),
        database,
        llama,
        gliner,
    }
}

/// Probe the database with a cheap round trip.
pub async fn probe_database(db: &DatabaseConnection) -> DependencyDto {
    match db.ping().await {
        Ok(()) => DependencyDto {
            key: "database".to_string(),
            label: "Database".to_string(),
            reachable: true,
            detail: None,
        },
        Err(err) => {
            tracing::warn!(error = %err, "the database does not answer");
            DependencyDto {
                key: "database".to_string(),
                label: "Database".to_string(),
                reachable: false,
                detail: Some(err.to_string()),
            }
        }
    }
}

/// Probe the llama.cpp server and read the models it offers.
async fn probe_llama(state: &AppState, settings: &SettingsDto) -> LlamaDependencyDto {
    let base_url = settings.resolver_base_url.clone();
    let model = settings.resolver_model.clone();

    match state
        .stores
        .resolver
        .client()
        .list_models(&base_url, LLAMA_PROBE_TIMEOUT)
        .await
    {
        Ok(models) => LlamaDependencyDto {
            reachable: true,
            detail: None,
            base_url,
            model,
            models,
        },
        Err(err) => {
            tracing::debug!(error = %err, "the llama.cpp server does not answer");
            LlamaDependencyDto {
                reachable: false,
                detail: Some(err.to_string()),
                base_url,
                model,
                models: Vec::new(),
            }
        }
    }
}

/// Report whether the built in GLiNER resolver can read a message.
///
/// The resolver needs two things: the selected model on disk, and a device
/// that this build and this machine can run it on.
pub fn probe_gliner(store: &GlinerStore, settings: &SettingsDto) -> DependencyDto {
    let key = "gliner".to_string();
    let label = "Built in GLiNER".to_string();
    let spec = catalog::find(&settings.gliner_model).unwrap_or_else(catalog::default_spec);

    if !store.is_installed(spec) {
        return DependencyDto {
            key,
            label,
            reachable: false,
            detail: Some(format!("The model {} is not installed.", spec.name)),
        };
    }

    match active_device(LocalDevice::from_stored(&settings.gliner_device)) {
        Ok(device) => DependencyDto {
            key,
            label,
            reachable: true,
            detail: Some(format!("Ready on the device {device}.")),
        },
        Err(err) => DependencyDto {
            key,
            label,
            reachable: false,
            detail: Some(err.to_string()),
        },
    }
}

/// Read the values the daemon starts from.
pub fn configured_settings(config: &CoreConfig) -> SettingsDto {
    let gliner = &config.resolver.gliner;
    SettingsDto {
        queue_enabled: config.queue.enabled,
        resolver_backend: config.resolver.backend.as_str().to_string(),
        resolver_base_url: config.resolver.base_url.clone(),
        resolver_model: config.resolver.model.clone(),
        gliner_model: gliner.model.clone(),
        gliner_device: gliner.device.as_str().to_string(),
        gliner_threshold: gliner.threshold,
        router_fast_path: config.resolver.router.fast_path,
        router_retrieve: config.resolver.router.retrieve.as_str().to_string(),
        router_decide: config.resolver.router.decide.as_str().to_string(),
        router_extract: config.resolver.router.extract.as_str().to_string(),
        router_top_k: config.resolver.router.top_k,
        router_floor: config.resolver.router.floor,
        router_margin: config.resolver.router.margin,
        router_lexical_weight: config.resolver.router.lexical_weight,
        router_dense_weight: config.resolver.router.dense_weight,
        router_embed_model: config.resolver.router.embed_model.clone(),
        router_models_dir: config.resolver.router.models_dir.display().to_string(),
        router_embed_source: config.resolver.router.embed_source.as_str().to_string(),
        router_embed_local_model: config.resolver.router.embed_local_model.clone(),
        router_rerank_model: config.resolver.router.rerank_model.clone(),
        router_local_device: config.resolver.router.local_device.as_str().to_string(),
        router_phrase_gate: config.resolver.router.phrase_gate,
        router_list_match: config.resolver.router.list_match.as_str().to_string(),
        router_list_floor: config.resolver.router.list_floor,
        preview_sentences: Vec::new(),
    }
}

/// Map one stored settings value to the reply of the endpoints.
fn to_dto(values: &crate::settings::SettingsValues) -> SettingsDto {
    SettingsDto {
        queue_enabled: values.queue_enabled,
        resolver_backend: values.resolver_backend.as_str().to_string(),
        resolver_base_url: values.resolver_base_url.clone(),
        resolver_model: values.resolver_model.clone(),
        gliner_model: values.gliner_model.clone(),
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
        router_embed_model: values.router.embed_model.clone(),
        router_models_dir: values.router.models_dir.display().to_string(),
        router_embed_source: values.router.embed_source.as_str().to_string(),
        router_embed_local_model: values.router.embed_local_model.clone(),
        router_rerank_model: values.router.rerank_model.clone(),
        router_local_device: values.router.local_device.as_str().to_string(),
        router_phrase_gate: values.router.phrase_gate,
        router_list_match: values.router.list_match.as_str().to_string(),
        router_list_floor: values.router.list_floor,
        preview_sentences: values.preview_sentences.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::config::RouterConfig;
    use sea_orm::Database;

    /// Build the settings of a probe test.
    fn settings(model: &str, device: &str) -> SettingsDto {
        let router = CoreConfig::default().resolver.router;
        settings_of(model, device, &router)
    }

    /// Build the settings of a probe test with the given router stages.
    fn settings_of(model: &str, device: &str, router: &RouterConfig) -> SettingsDto {
        SettingsDto {
            queue_enabled: true,
            resolver_backend: "hybrid".to_string(),
            resolver_base_url: "http://127.0.0.1:8012/v1".to_string(),
            resolver_model: "qwen3.5-4b".to_string(),
            gliner_model: model.to_string(),
            gliner_device: device.to_string(),
            gliner_threshold: 0.3,
            router_fast_path: router.fast_path,
            router_retrieve: router.retrieve.as_str().to_string(),
            router_decide: router.decide.as_str().to_string(),
            router_extract: router.extract.as_str().to_string(),
            router_top_k: router.top_k,
            router_floor: router.floor,
            router_margin: router.margin,
            router_lexical_weight: router.lexical_weight,
            router_dense_weight: router.dense_weight,
            router_embed_model: router.embed_model.clone(),
            router_models_dir: router.models_dir.display().to_string(),
            router_embed_source: router.embed_source.as_str().to_string(),
            router_embed_local_model: router.embed_local_model.clone(),
            router_rerank_model: router.rerank_model.clone(),
            router_local_device: router.local_device.as_str().to_string(),
            router_phrase_gate: router.phrase_gate,
            router_list_match: router.list_match.as_str().to_string(),
            router_list_floor: router.list_floor,
            preview_sentences: Vec::new(),
        }
    }

    #[tokio::test]
    async fn probe_database_reports_a_connection_that_answers() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("an in memory database opens");

        let probe = probe_database(&db).await;

        assert!(probe.reachable);
        assert_eq!(probe.key, "database");
        assert!(probe.detail.is_none());
    }

    #[tokio::test]
    async fn probe_database_reports_a_connection_that_is_gone() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("an in memory database opens");
        // Closing one handle closes the pool behind every handle.
        db.clone().close().await.expect("the connection closes");

        let probe = probe_database(&db).await;

        assert!(!probe.reachable);
        assert!(probe.detail.is_some());
    }

    #[test]
    fn probe_gliner_reports_the_disabled_case_reachable() {
        let store = GlinerStore::new(
            std::env::temp_dir().join(format!("alice-deps-{}", uuid::Uuid::new_v4())),
        );

        let probe = probe_gliner(&store, &settings("gliner_small-v2.1", "auto"));

        assert!(!probe.reachable);
        assert!(probe
            .detail
            .is_some_and(|detail| detail.contains("is not installed")));
    }

    #[test]
    fn probe_gliner_reports_a_ready_model_with_its_device() {
        let root = std::env::temp_dir().join(format!("alice-deps-{}", uuid::Uuid::new_v4()));
        let store = GlinerStore::new(root.clone());
        let spec = catalog::find("gliner_small-v2.1").expect("the model is in the catalog");
        for file in spec.files {
            let path = store.dir(spec.id).join(file.path);
            std::fs::create_dir_all(path.parent().expect("the file has a directory"))
                .expect("the directory is created");
            std::fs::write(&path, b"graph").expect("the file is written");
        }

        let probe = probe_gliner(&store, &settings(spec.id, "cpu"));

        assert!(probe.reachable);
        assert!(probe.detail.is_some_and(|detail| detail.contains("cpu")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn configured_settings_reads_the_configuration() {
        let config = CoreConfig::default();

        let dto = configured_settings(&config);

        assert_eq!(dto.resolver_backend, "hybrid");
        assert_eq!(dto.resolver_base_url, "http://127.0.0.1:8012/v1");
        assert_eq!(dto.gliner_model, "gliner_small-v2.1");
        assert_eq!(dto.gliner_device, "auto");
        assert!(dto.queue_enabled);
    }
}
