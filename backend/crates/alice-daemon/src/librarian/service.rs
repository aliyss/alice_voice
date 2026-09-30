//! Service of the librarian.
//!
//! The service is the one door to the memory. The turn writes an episode
//! and the worker reads it with the librarian model, so the daemon never
//! blocks a reply on a memory write. The read path answers a query and
//! the seed of a turn, so the model knows what the memory holds before it
//! asks for a fact.

use std::sync::Arc;
use std::time::Duration;

use std::collections::HashMap;

use alice_core::config::CoreConfig;
use alice_core::dto::{
    LibrarianLintDto, LibrarianStatusDto, MemoryNodeDto, MessageMemoryDto, SystemEventPayloadDto,
};
use chrono::{DateTime, Utc};
use sea_orm::DbErr;
use uuid::Uuid;

use crate::event_bus::EventBus;
use crate::librarian::extract::{build_extract_prompt, parse_extraction, EXTRACT_SCHEMA_NAME};
use crate::librarian::facts::{FactWrite, DEFAULT_IMPORTANCE};
use crate::librarian::store::LibrarianStore;
use crate::resolver::client::{AnswerRequest, LlamaClient};
use crate::settings::SettingsService;

/// The largest number of nodes a query returns.
const MAX_QUERY_LIMIT: u64 = 200;

/// The number of nodes a query returns when the request names none.
const DEFAULT_QUERY_LIMIT: u64 = 50;

/// How many times the worker reads one episode before it leaves it behind.
///
/// A model server may be restarted, so a turn that taught nothing is worth
/// another try. A turn that still fails after a few of them is kept with
/// the reason rather than retried for ever.
const MAX_INGEST_ATTEMPTS: i32 = 3;

/// How long the worker waits after the first failure of one episode.
///
/// The wait grows with every failure, so a server that is down for an hour
/// is asked a handful of times rather than on every pass of the loop.
const RETRY_BACKOFF_SECS: i64 = 30;

/// Service that reads and writes the long term memory.
#[derive(Clone, Debug)]
pub struct LibrarianService {
    store: LibrarianStore,
    client: LlamaClient,
    settings: SettingsService,
    config: Arc<CoreConfig>,
    events: EventBus,
}

impl LibrarianService {
    /// Create a new librarian service.
    pub fn new(
        store: LibrarianStore,
        client: LlamaClient,
        settings: SettingsService,
        config: Arc<CoreConfig>,
        events: EventBus,
    ) -> Self {
        Self {
            store,
            client,
            settings,
            config,
            events,
        }
    }

    /// Borrow the memory store.
    pub fn store(&self) -> &LibrarianStore {
        &self.store
    }

    /// The state of the librarian as the settings page sees it.
    pub async fn status(&self) -> Result<LibrarianStatusDto, DbErr> {
        let values = self.settings.get_settings().await?;
        let enabled = values.librarian_enabled;
        let base_url = values.librarian_base_url;
        let model = values.librarian_model;
        let (reachable, detail) = self.reachability(enabled, &base_url).await;
        Ok(LibrarianStatusDto {
            enabled,
            base_url,
            model,
            reachable,
            detail,
            nodes: self.store.count_nodes().await?,
            edges: self.store.count_edges().await?,
            current_edges: self.store.count_current_edges().await?,
            confirmed_edges: self.store.count_confirmed_edges().await?,
            pending_episodes: self.store.count_episodes(false).await?,
            ingested_episodes: self.store.count_episodes(true).await?,
        })
    }

    /// Search the memory for the nodes a text names.
    pub async fn query(&self, text: &str, limit: Option<u64>) -> Result<Vec<MemoryNodeDto>, DbErr> {
        let limit = limit
            .unwrap_or(DEFAULT_QUERY_LIMIT)
            .clamp(1, MAX_QUERY_LIMIT);
        self.store.query(text, limit).await
    }

    /// Write one node and its facts by hand.
    ///
    /// The write comes from a reader rather than from the model, so it
    /// carries the certainty of a person: the fact is sure and the words
    /// are the words of the editor. The gate belongs to the model, which
    /// may repeat what it holds, rather than to the human who owns the
    /// memory.
    pub async fn write(
        &self,
        key: &str,
        title: &str,
        body: &str,
        facts: &[(String, String)],
    ) -> Result<MemoryNodeDto, DbErr> {
        let now = Utc::now();
        let id = self.store.upsert_node(key, title, body, now).await?;
        self.store.set_node_content(id, title, body, now).await?;
        for (relation, value) in facts {
            self.store
                .add_fact(
                    id,
                    FactWrite {
                        relation,
                        value,
                        confidence: 1.0,
                        importance: DEFAULT_IMPORTANCE,
                        episode: None,
                    },
                    now,
                )
                .await?;
        }
        self.store
            .find_node(id)
            .await?
            .ok_or_else(|| DbErr::RecordNotFound(format!("memory node {key}")))
    }

    /// Move every fact and name of one concept onto another.
    ///
    /// The merge is for the concepts the memory holds twice. It answers
    /// with the concept that absorbed the other, or null when one of the
    /// two does not exist.
    pub async fn merge_nodes(
        &self,
        source: Uuid,
        target: Uuid,
    ) -> Result<Option<MemoryNodeDto>, DbErr> {
        if !self.store.merge_nodes(source, target, Utc::now()).await? {
            return Ok(None);
        }
        self.store.find_node(target).await
    }

    /// Forget the words of the turns the memory read long ago.
    ///
    /// The facts stay: the raw turn is the bulk of the memory and the risk
    /// of it, and it is worth keeping only while it waits for the worker
    /// and while a reader may want to see what it taught. A retention of
    /// zero days keeps every turn for ever.
    pub async fn maintain(&self) -> Result<u64, DbErr> {
        let days = self.config.librarian.episode_retention_days;
        if days == 0 {
            return Ok(0);
        }
        let before = Utc::now() - chrono::Duration::days(days as i64);
        self.store.prune_episodes(before).await
    }

    /// Read one concept and its facts by its identifier.
    pub async fn find_node(&self, id: Uuid) -> Result<Option<MemoryNodeDto>, DbErr> {
        self.store.find_node(id).await
    }

    /// Retire one fact and return the concept it belongs to.
    ///
    /// The fact is closed rather than deleted, so the memory still answers
    /// a question about the past. A fact no concept carries is null.
    pub async fn retire_fact(&self, fact_id: Uuid) -> Result<Option<MemoryNodeDto>, DbErr> {
        let Some(subject) = self.store.retire_fact(fact_id, Utc::now()).await? else {
            return Ok(None);
        };
        self.store.find_node(subject).await
    }

    /// Delete one concept and every fact of it.
    pub async fn delete_node(&self, id: Uuid) -> Result<bool, DbErr> {
        self.store.delete_node(id).await
    }

    /// Report the nodes no relation reaches and the closed relations.
    pub async fn lint(&self) -> Result<LibrarianLintDto, DbErr> {
        self.store.lint(MAX_QUERY_LIMIT).await
    }

    /// Read the seed a turn carries into its prompt.
    ///
    /// The seed is small on purpose: it tells the model that the memory
    /// exists and what it holds, and the model asks for the detail. The
    /// turn is what decides which concepts it shows, so a question about
    /// the editor reads what the memory holds about the editor rather
    /// than the newest concepts of the store. A memory that is off, or a
    /// store that does not answer, seeds nothing, so a turn never fails
    /// on the librarian.
    pub async fn seed(&self, text: &str) -> String {
        let Ok(values) = self.settings.get_settings().await else {
            return String::new();
        };
        if !values.librarian_enabled {
            return String::new();
        }
        let limit = self.config.librarian.context_nodes;
        self.store.seed(text, limit).await.unwrap_or_default()
    }

    /// Store one turn for the worker to read.
    pub async fn enqueue(
        &self,
        id: Uuid,
        message_id: Option<Uuid>,
        text: &str,
        reply: &str,
        intent: Option<&str>,
        at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let values = self.settings.get_settings().await?;
        if !values.librarian_enabled {
            return Ok(());
        }
        self.store
            .enqueue_episode(id, message_id, text, reply, intent, at)
            .await
    }

    /// Read what the memory learned from each of the given messages.
    ///
    /// The transcript marks the message the memory learned from, so the
    /// page reads the facts of the memory beside the turns it shows.
    pub async fn memory_of_messages(
        &self,
        messages: &[Uuid],
    ) -> Result<HashMap<Uuid, MessageMemoryDto>, DbErr> {
        let learned = self.store.memory_of_messages(messages).await?;
        Ok(learned
            .into_iter()
            .map(|(message, facts)| (message, MessageMemoryDto { facts }))
            .collect())
    }

    /// Read the oldest episode and write the facts it teaches.
    ///
    /// The function returns true when it read one episode. A model that
    /// does not answer keeps the turn for another attempt after a growing
    /// wait, so a server that is restarted teaches the turn it missed. A
    /// turn that still fails after a few attempts is kept with the reason,
    /// so a server that stays down grows the queue only that far.
    pub async fn ingest_once(&self) -> Result<bool, DbErr> {
        let Some(episode) = self.store.next_episode().await? else {
            return Ok(false);
        };
        let values = self.settings.get_settings().await?;
        if !values.librarian_enabled {
            self.store
                .finish_episode(episode.id, Some("the librarian is off"))
                .await?;
            return Ok(false);
        }
        let at = episode.created_at.with_timezone(&Utc);
        // The reader reuses a key it can see, so the prompt carries the
        // concepts the memory already holds. A store that does not answer
        // names nothing: the turn is still read, it just opens the subject
        // itself rather than enriching the concept of it.
        let known = self
            .store
            .list_nodes(self.config.librarian.context_nodes as u64)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|node| (node.key, node.title))
            .collect::<Vec<_>>();
        let prompt = build_extract_prompt(
            &episode.text,
            &episode.reply,
            episode.intent.as_deref(),
            &known,
        );
        let result = self
            .client
            .read_answer(
                &AnswerRequest {
                    base_url: values.librarian_base_url.clone(),
                    model: values.librarian_model.clone(),
                    system: prompt.system,
                    user: prompt.user,
                    answer_schema: prompt.answer_schema,
                    schema_name: EXTRACT_SCHEMA_NAME.to_string(),
                    max_tokens: self.config.librarian.max_tokens,
                    thinking: false,
                    timeout: Duration::from_secs(self.config.librarian.timeout_secs),
                },
                &mut |_delta| {},
            )
            .await;
        match result {
            Ok(content) => {
                // 1. Write every fact the reader taught the memory. The
                //    gate reads each fact first, and the write answers
                //    whether it changed anything, so a turn that only
                //    repeats what the memory holds reports nothing while it
                //    still counts as a sighting of those facts.
                let saved = self
                    .store
                    .apply_facts(episode.id, &parse_extraction(&content), at)
                    .await?;
                // 2. Tell the surface, so the transcript marks the message
                //    the memory learned from while the reader is looking at
                //    it. An episode of no stored turn has no message to
                //    mark, and a turn that taught nothing new marks none.
                if let (Some(message_id), false) = (episode.message_id, saved.is_empty()) {
                    self.events.publish(SystemEventPayloadDto::MemorySaved {
                        message_id,
                        facts: saved,
                    });
                }
                self.store.finish_episode(episode.id, None).await?;
                Ok(true)
            }
            Err(err) => {
                tracing::warn!(error = %err, "the librarian could not read one episode");
                let attempts = episode.attempts + 1;
                if attempts >= MAX_INGEST_ATTEMPTS {
                    // The turn waited long enough: the memory keeps it with
                    // the reason rather than asking for it for ever.
                    self.store
                        .finish_episode(episode.id, Some(&err.to_string()))
                        .await?;
                } else {
                    let wait = RETRY_BACKOFF_SECS * 3_i64.pow(attempts as u32 - 1);
                    let wait_until = Utc::now() + chrono::Duration::seconds(wait);
                    self.store
                        .retry_episode(&episode, &err.to_string(), wait_until)
                        .await?;
                }
                Ok(true)
            }
        }
    }

    /// Ask the librarian server whether it answers.
    async fn reachability(&self, enabled: bool, base_url: &str) -> (bool, Option<String>) {
        if !enabled {
            return (false, Some("The librarian is off.".to_string()));
        }
        let timeout = Duration::from_secs(self.config.librarian.timeout_secs.min(5));
        match self.client.list_models(base_url, timeout).await {
            Ok(_) => (true, None),
            Err(err) => (false, Some(err.to_string())),
        }
    }
}
