//! Settings service of the daemon.
//! This module reads and writes the values the settings page controls.
//!
//! The rows live in memory once the daemon has read them: a turn reads
//! five settings and the settings page reads them after every write, and
//! a write replaces the copy. A read of the database happens only when
//! the copy is missing, so a turn never waits for the settings table.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use alice_core::config::{
    CoreConfig, DecideEngine, EmbedSource, ExtractEngine, ListMatch, LocalDevice, ResolverBackend,
    RetrieveEngine, RouterConfig,
};
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};

use crate::settings::entity::setting;

/// Key of the queue toggle in the settings table.
const QUEUE_ENABLED_KEY: &str = "queue_enabled";

/// Key of the resolver backend in the settings table.
const RESOLVER_BACKEND_KEY: &str = "resolver_backend";

/// Key of the resolver base URL in the settings table.
const RESOLVER_BASE_URL_KEY: &str = "resolver_base_url";

/// Key of the resolver model name in the settings table.
const RESOLVER_MODEL_KEY: &str = "resolver_model";

/// Key of the GLiNER model in the settings table.
const GLINER_MODEL_KEY: &str = "gliner_model";

/// Key of the GLiNER device in the settings table.
const GLINER_DEVICE_KEY: &str = "gliner_device";

/// Key of the GLiNER threshold in the settings table.
const GLINER_THRESHOLD_KEY: &str = "gliner_threshold";

/// Keys of the layered router in the settings table.
///
/// One key holds one stage of the router, so a stage is stored and read on
/// its own and a page that shows one stage writes one value.
const ROUTER_FAST_PATH_KEY: &str = "router_fast_path";
const ROUTER_RETRIEVE_KEY: &str = "router_retrieve";
const ROUTER_DECIDE_KEY: &str = "router_decide";
const ROUTER_EXTRACT_KEY: &str = "router_extract";
const ROUTER_TOP_K_KEY: &str = "router_top_k";
const ROUTER_FLOOR_KEY: &str = "router_floor";
const ROUTER_MARGIN_KEY: &str = "router_margin";
const ROUTER_LEXICAL_WEIGHT_KEY: &str = "router_lexical_weight";
const ROUTER_DENSE_WEIGHT_KEY: &str = "router_dense_weight";
const ROUTER_EMBED_MODEL_KEY: &str = "router_embed_model";
const ROUTER_MODELS_DIR_KEY: &str = "router_models_dir";
const ROUTER_EMBED_SOURCE_KEY: &str = "router_embed_source";
const ROUTER_EMBED_LOCAL_MODEL_KEY: &str = "router_embed_local_model";
const ROUTER_RERANK_MODEL_KEY: &str = "router_rerank_model";
const ROUTER_LAYA_MODEL_KEY: &str = "router_laya_model";
const ROUTER_LOCAL_DEVICE_KEY: &str = "router_local_device";
const ROUTER_PHRASE_GATE_KEY: &str = "router_phrase_gate";
const ROUTER_LIST_MATCH_KEY: &str = "router_list_match";
const ROUTER_LIST_FLOOR_KEY: &str = "router_list_floor";
const ROUTER_FALLBACK_LLM_KEY: &str = "router_fallback_llm";
const ROUTER_SCRIPT_FALLBACK_KEY: &str = "router_script_fallback";
const ROUTER_OPEN_VALUES_LLM_KEY: &str = "router_open_values_llm";

/// Keys of the librarian in the settings table.
///
/// The librarian reads its own model, so a user may point it at a larger
/// model than the resolver runs and keep the reply fast.
const LIBRARIAN_ENABLED_KEY: &str = "librarian_enabled";
const LIBRARIAN_BASE_URL_KEY: &str = "librarian_base_url";
const LIBRARIAN_MODEL_KEY: &str = "librarian_model";

const RESPONSE_QUALITY_KEY: &str = "response_quality";
const RESPONSE_SPEED_KEY: &str = "response_speed";

/// Key of the sentences the settings page tries against the resolver.
///
/// The sentences are the tests of a user and not a stage of the pipeline,
/// so they are one list of the daemon and not a field of the router. They
/// are stored as one JSON array, because a list is what the page reads
/// back and writes whole.
const PREVIEW_SENTENCES_KEY: &str = "preview_sentences";

/// The values the settings page controls.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsValues {
    /// Whether the queue stores messages.
    pub queue_enabled: bool,
    /// The engine that reads the intent of a message.
    pub resolver_backend: ResolverBackend,
    /// The base URL of the intent resolver.
    pub resolver_base_url: String,
    /// The model name the resolver asks the server for.
    pub resolver_model: String,
    /// Identifier of the built in GLiNER model.
    pub gliner_model: String,
    /// The device a GLiNER model runs on.
    pub gliner_device: LocalDevice,
    /// Smallest probability a GLiNER label needs to count.
    pub gliner_threshold: f32,
    /// The stages of the layered router.
    pub router: RouterConfig,
    /// Response quality 0..100. 0 is fastest, 100 is best. Speed is 100 - quality.
    pub response_quality: u8,
    /// Response speed 0..100. Always 100 - quality.
    pub response_speed: u8,
    /// The sentences the settings page tries against the resolver.
    pub preview_sentences: Vec<String>,
    /// Whether the librarian keeps a long term memory.
    pub librarian_enabled: bool,
    /// The base URL of the model server the librarian reads.
    pub librarian_base_url: String,
    /// The model the librarian reads.
    pub librarian_model: String,
}

/// Service that persists the daemon settings.
#[derive(Clone, Debug)]
pub struct SettingsService {
    db: DatabaseConnection,
    defaults: SettingsValues,
    /// The rows the daemon read last, or none before the first read.
    cache: Arc<Mutex<Option<SettingsValues>>>,
}

impl SettingsService {
    /// Create a new settings service.
    pub fn new(db: DatabaseConnection, defaults: SettingsValues) -> Self {
        Self {
            db,
            defaults,
            cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Lock the copy of the rows.
    fn lock(&self) -> MutexGuard<'_, Option<SettingsValues>> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Forget the copy, so the next read takes the rows from the database.
    fn forget(&self) {
        *self.lock() = None;
    }

    /// Build the settings defaults out of the configuration file.
    pub fn defaults(config: &CoreConfig) -> SettingsValues {
        SettingsValues {
            queue_enabled: config.queue.enabled,
            resolver_backend: config.resolver.backend,
            resolver_base_url: config.resolver.base_url.clone(),
            resolver_model: config.resolver.model.clone(),
            gliner_model: config.resolver.gliner.model.clone(),
            gliner_device: config.resolver.gliner.device,
            gliner_threshold: config.resolver.gliner.threshold,
            router: config.resolver.router.clone(),
            response_quality: 50,
            response_speed: 50,
            preview_sentences: Vec::new(),
            librarian_enabled: config.librarian.enabled,
            librarian_base_url: config.librarian.base_url.clone(),
            librarian_model: config.librarian.model.clone(),
        }
    }

    /// Read every setting from memory, or from the database once.
    ///
    /// A key without a row returns its default.
    pub async fn get_settings(&self) -> Result<SettingsValues, sea_orm::DbErr> {
        if let Some(values) = self.lock().clone() {
            return Ok(values);
        }
        let values = self.read_settings().await?;
        *self.lock() = Some(values.clone());
        Ok(values)
    }

    /// Read every setting out of the table.
    async fn read_settings(&self) -> Result<SettingsValues, sea_orm::DbErr> {
        let quality_opt = self
            .get_number::<u8>(RESPONSE_QUALITY_KEY)
            .await?
            .map(|value| value.min(100));
        let speed_opt = self
            .get_number::<u8>(RESPONSE_SPEED_KEY)
            .await?
            .map(|value| value.min(100));
        let (response_quality, response_speed) = match (quality_opt, speed_opt) {
            (Some(quality), _) => (quality, 100 - quality),
            (None, Some(speed)) => (100 - speed, speed),
            (None, None) => (self.defaults.response_quality, self.defaults.response_speed),
        };
        Ok(SettingsValues {
            queue_enabled: self
                .get_flag(QUEUE_ENABLED_KEY)
                .await?
                .unwrap_or(self.defaults.queue_enabled),
            resolver_backend: self
                .get_text(RESOLVER_BACKEND_KEY)
                .await?
                .map(|value| ResolverBackend::from_stored(&value))
                .unwrap_or(self.defaults.resolver_backend),
            resolver_base_url: self
                .get_text(RESOLVER_BASE_URL_KEY)
                .await?
                .unwrap_or_else(|| self.defaults.resolver_base_url.clone()),
            resolver_model: self
                .get_text(RESOLVER_MODEL_KEY)
                .await?
                .unwrap_or_else(|| self.defaults.resolver_model.clone()),
            gliner_model: self
                .get_text(GLINER_MODEL_KEY)
                .await?
                .unwrap_or_else(|| self.defaults.gliner_model.clone()),
            gliner_device: self
                .get_text(GLINER_DEVICE_KEY)
                .await?
                .map(|value| LocalDevice::from_stored(&value))
                .unwrap_or(self.defaults.gliner_device),
            gliner_threshold: self
                .get_text(GLINER_THRESHOLD_KEY)
                .await?
                .and_then(|value| value.parse().ok())
                .unwrap_or(self.defaults.gliner_threshold),
            router: self.read_router().await?,
            response_quality,
            response_speed,
            preview_sentences: self
                .get_text(PREVIEW_SENTENCES_KEY)
                .await?
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_default(),
            librarian_enabled: self
                .get_flag(LIBRARIAN_ENABLED_KEY)
                .await?
                .unwrap_or(self.defaults.librarian_enabled),
            librarian_base_url: self
                .get_text(LIBRARIAN_BASE_URL_KEY)
                .await?
                .unwrap_or_else(|| self.defaults.librarian_base_url.clone()),
            librarian_model: self
                .get_text(LIBRARIAN_MODEL_KEY)
                .await?
                .unwrap_or_else(|| self.defaults.librarian_model.clone()),
        })
    }

    /// Write the sentences the settings page tries against the resolver.
    ///
    /// A value the daemon cannot write leaves the stored list alone: the
    /// sentences are a scratch pad, and losing them is worse than writing
    /// nothing.
    pub async fn set_preview_sentences(&self, sentences: &[String]) -> Result<(), sea_orm::DbErr> {
        let encoded = serde_json::to_string(sentences).unwrap_or_else(|_| "[]".to_string());
        self.put(PREVIEW_SENTENCES_KEY, encoded).await
    }

    /// Read the response quality. 0 is fastest, 100 is best.
    pub async fn get_response_quality(&self) -> Result<u8, sea_orm::DbErr> {
        Ok(self.get_settings().await?.response_quality)
    }

    /// Write the response quality and keep speed linked.
    pub async fn set_response_quality(&self, quality: u8) -> Result<(), sea_orm::DbErr> {
        let quality = quality.min(100);
        let speed = 100 - quality;
        self.put(RESPONSE_QUALITY_KEY, quality.to_string()).await?;
        self.put(RESPONSE_SPEED_KEY, speed.to_string()).await
    }

    /// Read the response speed. 0 is slowest, 100 is fastest.
    pub async fn get_response_speed(&self) -> Result<u8, sea_orm::DbErr> {
        Ok(self.get_settings().await?.response_speed)
    }

    /// Write the response speed and keep quality linked.
    pub async fn set_response_speed(&self, speed: u8) -> Result<(), sea_orm::DbErr> {
        let speed = speed.min(100);
        let quality = 100 - speed;
        self.put(RESPONSE_SPEED_KEY, speed.to_string()).await?;
        self.put(RESPONSE_QUALITY_KEY, quality.to_string()).await
    }

    /// Read the stages of the layered router out of the table.
    async fn read_router(&self) -> Result<RouterConfig, sea_orm::DbErr> {
        let defaults = &self.defaults.router;
        Ok(RouterConfig {
            fast_path: self
                .get_flag(ROUTER_FAST_PATH_KEY)
                .await?
                .unwrap_or(defaults.fast_path),
            retrieve: self
                .get_text(ROUTER_RETRIEVE_KEY)
                .await?
                .map(|value| RetrieveEngine::from_stored(&value))
                .unwrap_or(defaults.retrieve),
            decide: self
                .get_text(ROUTER_DECIDE_KEY)
                .await?
                .map(|value| DecideEngine::from_stored(&value))
                .unwrap_or(defaults.decide),
            extract: self
                .get_text(ROUTER_EXTRACT_KEY)
                .await?
                .map(|value| ExtractEngine::from_stored(&value))
                .unwrap_or(defaults.extract),
            top_k: self
                .get_number(ROUTER_TOP_K_KEY)
                .await?
                .unwrap_or(defaults.top_k),
            floor: self
                .get_number(ROUTER_FLOOR_KEY)
                .await?
                .unwrap_or(defaults.floor),
            margin: self
                .get_number(ROUTER_MARGIN_KEY)
                .await?
                .unwrap_or(defaults.margin),
            lexical_weight: self
                .get_number(ROUTER_LEXICAL_WEIGHT_KEY)
                .await?
                .unwrap_or(defaults.lexical_weight),
            dense_weight: self
                .get_number(ROUTER_DENSE_WEIGHT_KEY)
                .await?
                .unwrap_or(defaults.dense_weight),
            embed_model: self
                .get_text(ROUTER_EMBED_MODEL_KEY)
                .await?
                .unwrap_or_else(|| defaults.embed_model.clone()),
            models_dir: self
                .get_text(ROUTER_MODELS_DIR_KEY)
                .await?
                .map(PathBuf::from)
                .unwrap_or_else(|| defaults.models_dir.clone()),
            embed_source: self
                .get_text(ROUTER_EMBED_SOURCE_KEY)
                .await?
                .map(|value| EmbedSource::from_stored(&value))
                .unwrap_or(defaults.embed_source),
            embed_local_model: self
                .get_text(ROUTER_EMBED_LOCAL_MODEL_KEY)
                .await?
                .unwrap_or_else(|| defaults.embed_local_model.clone()),
            rerank_model: self
                .get_text(ROUTER_RERANK_MODEL_KEY)
                .await?
                .unwrap_or_else(|| defaults.rerank_model.clone()),
            laya_model: self
                .get_text(ROUTER_LAYA_MODEL_KEY)
                .await?
                .unwrap_or_else(|| defaults.laya_model.clone()),
            local_device: self
                .get_text(ROUTER_LOCAL_DEVICE_KEY)
                .await?
                .map(|value| LocalDevice::from_stored(&value))
                .unwrap_or(defaults.local_device),
            phrase_gate: self
                .get_flag(ROUTER_PHRASE_GATE_KEY)
                .await?
                .unwrap_or(defaults.phrase_gate),
            list_match: self
                .get_text(ROUTER_LIST_MATCH_KEY)
                .await?
                .map(|value| ListMatch::from_stored(&value))
                .unwrap_or(defaults.list_match),
            list_floor: self
                .get_number(ROUTER_LIST_FLOOR_KEY)
                .await?
                .unwrap_or(defaults.list_floor),
            fallback_llm: self
                .get_flag(ROUTER_FALLBACK_LLM_KEY)
                .await?
                .unwrap_or(defaults.fallback_llm),
            script_fallback: self
                .get_flag(ROUTER_SCRIPT_FALLBACK_KEY)
                .await?
                .unwrap_or(defaults.script_fallback),
            open_values_llm: self
                .get_flag(ROUTER_OPEN_VALUES_LLM_KEY)
                .await?
                .unwrap_or(defaults.open_values_llm),
        })
    }

    /// Read the stages of the layered router.
    pub async fn get_router(&self) -> Result<RouterConfig, sea_orm::DbErr> {
        Ok(self.get_settings().await?.router)
    }

    /// Write the stages of the layered router.
    pub async fn set_router(&self, config: &RouterConfig) -> Result<(), sea_orm::DbErr> {
        let flag = |value: bool| if value { "true" } else { "false" }.to_string();
        self.put(ROUTER_FAST_PATH_KEY, flag(config.fast_path))
            .await?;
        self.put(ROUTER_RETRIEVE_KEY, config.retrieve.as_str().to_string())
            .await?;
        self.put(ROUTER_DECIDE_KEY, config.decide.as_str().to_string())
            .await?;
        self.put(ROUTER_EXTRACT_KEY, config.extract.as_str().to_string())
            .await?;
        self.put(ROUTER_TOP_K_KEY, config.top_k.to_string()).await?;
        self.put(ROUTER_FLOOR_KEY, config.floor.to_string()).await?;
        self.put(ROUTER_MARGIN_KEY, config.margin.to_string())
            .await?;
        self.put(ROUTER_LEXICAL_WEIGHT_KEY, config.lexical_weight.to_string())
            .await?;
        self.put(ROUTER_DENSE_WEIGHT_KEY, config.dense_weight.to_string())
            .await?;
        self.put(ROUTER_EMBED_MODEL_KEY, config.embed_model.clone())
            .await?;
        self.put(
            ROUTER_MODELS_DIR_KEY,
            config.models_dir.display().to_string(),
        )
        .await?;
        self.put(
            ROUTER_EMBED_SOURCE_KEY,
            config.embed_source.as_str().to_string(),
        )
        .await?;
        self.put(
            ROUTER_EMBED_LOCAL_MODEL_KEY,
            config.embed_local_model.clone(),
        )
        .await?;
        self.put(ROUTER_RERANK_MODEL_KEY, config.rerank_model.clone())
            .await?;
        self.put(ROUTER_LAYA_MODEL_KEY, config.laya_model.clone())
            .await?;
        self.put(
            ROUTER_LOCAL_DEVICE_KEY,
            config.local_device.as_str().to_string(),
        )
        .await?;
        self.put(ROUTER_PHRASE_GATE_KEY, flag(config.phrase_gate))
            .await?;
        self.put(
            ROUTER_LIST_MATCH_KEY,
            config.list_match.as_str().to_string(),
        )
        .await?;
        self.put(ROUTER_LIST_FLOOR_KEY, config.list_floor.to_string())
            .await?;
        self.put(ROUTER_FALLBACK_LLM_KEY, flag(config.fallback_llm))
            .await?;
        self.put(ROUTER_SCRIPT_FALLBACK_KEY, flag(config.script_fallback))
            .await?;
        self.put(ROUTER_OPEN_VALUES_LLM_KEY, flag(config.open_values_llm))
            .await?;
        Ok(())
    }

    /// Read the resolver backend.
    pub async fn get_resolver_backend(&self) -> Result<ResolverBackend, sea_orm::DbErr> {
        Ok(self.get_settings().await?.resolver_backend)
    }

    /// Write the resolver backend.
    pub async fn set_resolver_backend(
        &self,
        backend: ResolverBackend,
    ) -> Result<(), sea_orm::DbErr> {
        self.put(RESOLVER_BACKEND_KEY, backend.as_str().to_string())
            .await
    }

    /// Read the identifier of the built in GLiNER model.
    pub async fn get_gliner_model(&self) -> Result<String, sea_orm::DbErr> {
        Ok(self.get_settings().await?.gliner_model)
    }

    /// Write the identifier of the built in GLiNER model.
    pub async fn set_gliner_model(&self, model: &str) -> Result<(), sea_orm::DbErr> {
        self.put(GLINER_MODEL_KEY, model.to_string()).await
    }

    /// Read the device the built in GLiNER model runs on.
    pub async fn get_gliner_device(&self) -> Result<LocalDevice, sea_orm::DbErr> {
        Ok(self.get_settings().await?.gliner_device)
    }

    /// Write the device the built in GLiNER model runs on.
    pub async fn set_gliner_device(&self, device: LocalDevice) -> Result<(), sea_orm::DbErr> {
        self.put(GLINER_DEVICE_KEY, device.as_str().to_string())
            .await
    }

    /// Read the smallest probability a GLiNER label needs to count.
    pub async fn get_gliner_threshold(&self) -> Result<f32, sea_orm::DbErr> {
        Ok(self.get_settings().await?.gliner_threshold)
    }

    /// Write the smallest probability a GLiNER label needs to count.
    pub async fn set_gliner_threshold(&self, threshold: f32) -> Result<(), sea_orm::DbErr> {
        self.put(GLINER_THRESHOLD_KEY, threshold.to_string()).await
    }

    /// Read the queue toggle. It returns the default when no row exists.
    pub async fn get_queue_enabled(&self) -> Result<bool, sea_orm::DbErr> {
        Ok(self.get_settings().await?.queue_enabled)
    }

    /// Write the queue toggle.
    pub async fn set_queue_enabled(&self, enabled: bool) -> Result<(), sea_orm::DbErr> {
        let value = if enabled { "true" } else { "false" }.to_string();
        self.put(QUEUE_ENABLED_KEY, value).await
    }

    /// Read the base URL of the intent resolver.
    pub async fn get_resolver_base_url(&self) -> Result<String, sea_orm::DbErr> {
        Ok(self.get_settings().await?.resolver_base_url)
    }

    /// Write the base URL of the intent resolver.
    pub async fn set_resolver_base_url(&self, url: &str) -> Result<(), sea_orm::DbErr> {
        self.put(RESOLVER_BASE_URL_KEY, url.to_string()).await
    }

    /// Read the model name of the intent resolver.
    pub async fn get_resolver_model(&self) -> Result<String, sea_orm::DbErr> {
        Ok(self.get_settings().await?.resolver_model)
    }

    /// Write the model name of the intent resolver.
    pub async fn set_resolver_model(&self, model: &str) -> Result<(), sea_orm::DbErr> {
        self.put(RESOLVER_MODEL_KEY, model.to_string()).await
    }

    /// Read whether the librarian keeps a long term memory.
    pub async fn get_librarian_enabled(&self) -> Result<bool, sea_orm::DbErr> {
        Ok(self.get_settings().await?.librarian_enabled)
    }

    /// Write whether the librarian keeps a long term memory.
    pub async fn set_librarian_enabled(&self, enabled: bool) -> Result<(), sea_orm::DbErr> {
        let value = if enabled { "true" } else { "false" }.to_string();
        self.put(LIBRARIAN_ENABLED_KEY, value).await
    }

    /// Read the base URL of the model server the librarian reads.
    pub async fn get_librarian_base_url(&self) -> Result<String, sea_orm::DbErr> {
        Ok(self.get_settings().await?.librarian_base_url)
    }

    /// Write the base URL of the model server the librarian reads.
    pub async fn set_librarian_base_url(&self, url: &str) -> Result<(), sea_orm::DbErr> {
        self.put(LIBRARIAN_BASE_URL_KEY, url.to_string()).await
    }

    /// Read the model the librarian reads.
    pub async fn get_librarian_model(&self) -> Result<String, sea_orm::DbErr> {
        Ok(self.get_settings().await?.librarian_model)
    }

    /// Write the model the librarian reads.
    pub async fn set_librarian_model(&self, model: &str) -> Result<(), sea_orm::DbErr> {
        self.put(LIBRARIAN_MODEL_KEY, model.to_string()).await
    }

    /// Read one text value.
    async fn get_text(&self, key: &str) -> Result<Option<String>, sea_orm::DbErr> {
        let row = setting::Entity::find_by_id(key.to_string())
            .one(&self.db)
            .await?;
        Ok(row.map(|row| row.value))
    }

    /// Read one boolean value.
    async fn get_flag(&self, key: &str) -> Result<Option<bool>, sea_orm::DbErr> {
        let value = self.get_text(key).await?;
        Ok(value.map(|value| value == "true" || value == "1"))
    }

    /// Read one number of a setting.
    async fn get_number<T: std::str::FromStr>(
        &self,
        key: &str,
    ) -> Result<Option<T>, sea_orm::DbErr> {
        Ok(self
            .get_text(key)
            .await?
            .and_then(|value| value.parse().ok()))
    }

    /// Write one value and replace the row when it exists.
    async fn put(&self, key: &str, value: String) -> Result<(), sea_orm::DbErr> {
        let active = setting::ActiveModel {
            key: Set(key.to_string()),
            value: Set(value),
        };
        setting::Entity::insert(active)
            .on_conflict(
                sea_orm::sea_query::OnConflict::column(setting::Column::Key)
                    .update_column(setting::Column::Value)
                    .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await?;
        self.forget();
        Ok(())
    }
}
