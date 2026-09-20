//! The built in GLiNER resolver.
//!
//! GLiNER reads a message once and reports the spans that match the labels
//! it is given. The resolver hands it one label per intent and one label
//! per entity, then reads the intent out of the scores and the values out
//! of the spans.
//!
//! Unlike the llama.cpp resolver this needs no server: the model runs in
//! the daemon, on the processor or on a CUDA capable graphics card.

pub mod catalog;
pub mod engine;
pub mod error;
pub mod scoring;
pub mod store;

use std::collections::BTreeMap;
use std::sync::Arc;

use alice_core::config::{LocalDevice, ResolverEngine};
use alice_core::dto::IntentDto;

use crate::intent::EntityScripts;
use crate::resolver::gliner::engine::{GlinerEngine, LabelHit};
use crate::resolver::service::{Resolution, ResolvedIntent};

pub use engine::{active_device, available_devices, cuda_build, DEVICE_CPU};
pub use error::GlinerError;
pub use store::GlinerStore;

/// The resolver that reads intents and entity values with one GLiNER model.
#[derive(Clone, Debug)]
pub struct GlinerResolver {
    engine: GlinerEngine,
}

impl GlinerResolver {
    /// Create a new resolver.
    pub fn new(store: Arc<GlinerStore>, threads: usize) -> Self {
        Self {
            engine: GlinerEngine::new(store, threads),
        }
    }

    /// Read the intent of one message.
    ///
    /// The message scores every intent at once: one inference reads the
    /// intent labels and the entity labels together, so the resolver stays
    /// one call per turn however many intents the user configured.
    pub async fn resolve(
        &self,
        intents: &[IntentDto],
        text: &str,
        model: &str,
        device: LocalDevice,
        threshold: f32,
        scripts: &EntityScripts,
    ) -> Result<Resolution, GlinerError> {
        // Every way out reports the engine and the model that read the
        // message, so a turn without an intent still names both.
        let unmatched = || Resolution::Unmatched {
            engine: ResolverEngine::Gliner,
            model: Some(model.to_string()),
            route: crate::resolver::service::RouteReport::default(),
        };
        let labels = scoring::labels_for(intents);
        let hits = self.hits(labels, text, model, device).await?;
        let scores = scoring::score_intents(intents, &hits, text);
        let Some(best) = scores.first() else {
            return Ok(unmatched());
        };
        if best.score < threshold {
            tracing::debug!(
                best = %best.name,
                score = best.score,
                threshold,
                "no intent reached the threshold"
            );
            return Ok(unmatched());
        }
        let Some(intent) = intents.get(best.index) else {
            return Ok(unmatched());
        };

        tracing::debug!(
            intent = %intent.name,
            score = best.score,
            evidence = best.evidence.as_deref().unwrap_or("own label"),
            "the built in GLiNER model read an intent"
        );
        // The values of the chosen intent are read on their own: the
        // labels of one intent are few, so the read stays small, and a
        // script entity offers its list when the list is short enough to
        // be a label.
        let lists = scripts.lists(intent).await;
        let offered = scripts.offered_values(&lists);
        let hits = self
            .hits(scoring::read_labels(intent, &offered), text, model, device)
            .await?;
        Ok(Resolution::Matched(read_intent(
            intent, &hits, text, best.score, &offered,
        )))
    }

    /// Read the values of the entities of one intent.
    ///
    /// Only the labels of the chosen intent reach the model, so the second
    /// read of a turn stays small however large the catalog is.
    pub async fn read_values(
        &self,
        intent: &IntentDto,
        text: &str,
        model: &str,
        device: LocalDevice,
        offered: &BTreeMap<String, Vec<String>>,
    ) -> Result<BTreeMap<String, String>, GlinerError> {
        if intent.entities.is_empty() {
            return Ok(BTreeMap::new());
        }
        let hits = self
            .hits(scoring::read_labels(intent, offered), text, model, device)
            .await?;
        Ok(scoring::read_values(intent, &hits, text, offered))
    }

    /// Run one inference on the blocking pool of the runtime.
    async fn hits(
        &self,
        labels: Vec<String>,
        text: &str,
        model: &str,
        device: LocalDevice,
    ) -> Result<Vec<LabelHit>, GlinerError> {
        let spec = self.engine.store().installed_spec(model)?;
        let engine = self.engine.clone();
        let text = text.to_string();
        tokio::task::spawn_blocking(move || engine.infer(spec, device, &text, &labels))
            .await
            .map_err(|err| GlinerError::Inference(err.to_string()))?
    }
}

/// Read one intent out of the hits of one message.
fn read_intent(
    intent: &IntentDto,
    hits: &[LabelHit],
    message: &str,
    confidence: f32,
    offered: &BTreeMap<String, Vec<String>>,
) -> ResolvedIntent {
    ResolvedIntent {
        id: intent.id,
        name: intent.name.clone(),
        command: intent.command.clone(),
        entities: intent
            .entities
            .iter()
            .map(|entity| entity.name.clone())
            .collect(),
        required: intent
            .entities
            .iter()
            .filter(|entity| entity.required)
            .map(|entity| entity.name.clone())
            .collect(),
        values: scoring::read_values(intent, hits, message, offered),
        // The service names the reader of every value once the lists of
        // the entities are read, so the model reports the values alone.
        origins: BTreeMap::new(),
        confidence: Some(confidence),
        intent_engine: ResolverEngine::Gliner,
        // The service owns the model names, because it is the layer that
        // read the settings of the turn.
        intent_model: None,
        value_engine: (!intent.entities.is_empty()).then_some(ResolverEngine::Gliner),
        value_model: None,
        route: crate::resolver::service::RouteReport::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::dto::IntentEntityDto;
    use chrono::Utc;
    use uuid::Uuid;

    /// Build one intent for the tests.
    fn intent(name: &str) -> IntentDto {
        IntentDto {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: String::new(),
            command: "echo hi".to_string(),
            entities: vec![IntentEntityDto {
                id: Uuid::new_v4(),
                name: "city".to_string(),
                kind: alice_core::dto::EntityKindDto::Open,
                values: Vec::new(),
                script: None,
                required: true,
            }],
            examples: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Build a script service that runs commands of the tests.
    fn scripts() -> EntityScripts {
        EntityScripts::new(
            crate::execution::CommandRunner::new(::std::time::Duration::from_secs(5), 4096),
            ::std::time::Duration::from_secs(60),
            200,
            24,
        )
    }

    #[test]
    fn read_intent_reads_the_span_of_each_entity() {
        let catalog = intent("get weather");
        let hits = vec![LabelHit {
            label: "city".to_string(),
            text: "Berlin".to_string(),
            score: 0.85,
        }];

        let resolved = read_intent(
            &catalog,
            &hits,
            "the weather in Berlin",
            0.85,
            &BTreeMap::new(),
        );

        assert_eq!(resolved.name, "get weather");
        assert_eq!(resolved.entities, vec!["city"]);
        assert_eq!(resolved.values["city"], "Berlin");
        assert_eq!(resolved.confidence, Some(0.85));
    }

    #[tokio::test]
    async fn a_model_that_is_not_installed_is_reported() {
        let root = std::env::temp_dir().join(format!("alice-gliner-{}", Uuid::new_v4()));
        let resolver = GlinerResolver::new(Arc::new(GlinerStore::new(root.clone())), 2);

        let err = resolver
            .resolve(
                &[intent("get weather")],
                "the weather in Berlin",
                "gliner_small-v2.1",
                LocalDevice::Cpu,
                0.3,
                &scripts(),
            )
            .await
            .expect_err("the model is not on disk");

        assert!(matches!(err, GlinerError::NotInstalled(_)));
        let _ = std::fs::remove_dir_all(root);
    }
}
