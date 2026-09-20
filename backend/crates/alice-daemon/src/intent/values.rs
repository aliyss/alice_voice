//! The values a script entity offers the resolver.
//!
//! The list of a script entity belongs to the turn and not to a table:
//! the daemon runs the script of the entity, keeps the values in memory
//! for the configured time, and reads the value of a message out of that
//! list. The database holds the script and not the values, so a list that
//! changes with the machine stays a live list.
//!
//! A run that fails does not lose the list the daemon already read: the
//! turn reads the older list and the failure is reported next to it. A
//! run that fails on a daemon that never read the list reports a reason
//! and no values, so the turn refuses rather than guesses.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use uuid::Uuid;

use alice_core::dto::{EntityKindDto, IntentDto, IntentEntityDto, ScriptPreviewDto};

use crate::execution::CommandRunner;

/// The values one script answered with.
#[derive(Clone, Debug)]
struct Cached {
    /// The script the values came from. A changed script is a new list.
    script: String,
    /// The values, in the order the script wrote them.
    values: Vec<String>,
    /// The time the daemon ran the script.
    at: Instant,
    /// Why the script did not answer, or none when it did.
    error: Option<String>,
}

impl Cached {
    /// Whether the entry is young enough to answer for the script.
    fn is_fresh(&self, now: Instant, ttl: Duration) -> bool {
        now.duration_since(self.at) < ttl
    }
}

/// The list of one entity, and why the daemon has none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityList {
    /// The values the script wrote.
    pub values: Vec<String>,
    /// Why the daemon read no values, or none when it read them.
    pub error: Option<String>,
}

impl EntityList {
    /// A list that answers every value.
    fn read(values: Vec<String>) -> Self {
        Self {
            values,
            error: None,
        }
    }

    /// A list the daemon could not read.
    fn failed(reason: String) -> Self {
        Self {
            values: Vec::new(),
            error: Some(reason),
        }
    }

    /// Whether the entity has values a message can name.
    pub fn is_readable(&self) -> bool {
        !self.values.is_empty()
    }
}

/// Service that runs the scripts of the script entities.
#[derive(Clone, Debug)]
pub struct EntityScripts {
    /// The runner of the scripts. It carries the script timeout.
    runner: CommandRunner,
    /// The lists the daemon read, by entity.
    cache: Arc<Mutex<HashMap<Uuid, Cached>>>,
    /// How long one list answers for its script.
    ttl: Duration,
    /// Largest number of values the daemon keeps from one script.
    max_values: usize,
    /// Largest number of values the daemon offers a model as labels.
    label_budget: usize,
}

impl EntityScripts {
    /// Create a new service.
    pub fn new(
        runner: CommandRunner,
        ttl: Duration,
        max_values: usize,
        label_budget: usize,
    ) -> Self {
        Self {
            runner,
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            max_values: max_values.max(1),
            label_budget,
        }
    }

    /// Lock the lists in memory.
    fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, Cached>> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The list of one entity.
    pub async fn list(&self, entity: &IntentEntityDto) -> EntityList {
        let Some(script) = entity.script.as_deref().map(str::trim) else {
            return EntityList::failed("the entity has no script".to_string());
        };
        if script.is_empty() {
            return EntityList::failed("the entity has no script".to_string());
        }

        let now = Instant::now();
        let stored = self.lock().get(&entity.id).cloned();
        if let Some(stored) = &stored {
            if stored.script == script && stored.is_fresh(now, self.ttl) {
                return EntityList {
                    values: stored.values.clone(),
                    error: stored.error.clone(),
                };
            }
        }

        let read = self.run(script).await;
        let list = match read {
            Ok(values) if !values.is_empty() => EntityList::read(values),
            Ok(_) => EntityList::failed("the script wrote no value".to_string()),
            // A script that failed keeps the list the daemon read last, so
            // a broken script does not break the turns of the user.
            Err(reason) => match stored {
                Some(stored) if !stored.values.is_empty() => {
                    tracing::warn!(
                        entity = %entity.name,
                        reason = %reason,
                        "the script of the entity failed, the daemon reads the older list"
                    );
                    EntityList {
                        values: stored.values,
                        error: Some(reason),
                    }
                }
                _ => EntityList::failed(reason),
            },
        };

        self.lock().insert(
            entity.id,
            Cached {
                script: script.to_string(),
                values: list.values.clone(),
                at: now,
                error: list.error.clone(),
            },
        );
        list
    }

    /// The lists of every script entity of one intent, by entity name.
    ///
    /// The service reads the lists of the entities that carry a script.
    /// A closed entity carries its values in the configuration and an
    /// open entity has none, so neither belongs here.
    pub async fn lists(&self, intent: &IntentDto) -> BTreeMap<String, EntityList> {
        let mut lists: BTreeMap<String, EntityList> = BTreeMap::new();
        for entity in &intent.entities {
            if !entity.kind.has_values() {
                continue;
            }
            let list = self.list(entity).await;
            if !list.is_readable() {
                if let Some(reason) = &list.error {
                    tracing::warn!(
                        intent = %intent.name,
                        entity = %entity.name,
                        reason = %reason,
                        "the daemon read no list for an entity"
                    );
                }
            }
            lists.insert(entity.name.clone(), list);
        }
        lists
    }

    /// The values the resolver offers an engine for one list.
    ///
    /// A vanilla model degrades past about thirty labels and its window is
    /// small, so a list longer than the budget is not offered: the model
    /// reads the words of the value and the daemon reads the entry of the
    /// list those words name. An empty list offers nothing.
    pub fn offered<'a>(&self, list: &'a EntityList) -> Option<&'a [String]> {
        if list.values.is_empty() || list.values.len() > self.label_budget {
            return None;
        }
        Some(&list.values)
    }

    /// Whether the entity offers its values as labels.
    ///
    /// A list inside the budget is offered like the values of a closed
    /// entity, so the model chooses one of them directly. A longer list is
    /// read by matching instead. The resolver reads the offered map, so
    /// this names the rule of one list for the tests of that rule.
    #[cfg(test)]
    pub fn offers_labels(&self, list: &EntityList) -> bool {
        self.offered(list).is_some()
    }

    /// The values the resolver offers an engine, by entity name.
    ///
    /// A list inside the budget is offered like the values of a closed
    /// entity, so the model chooses one of them directly. A longer list is
    /// read by matching instead, so it stays out of the map.
    pub fn offered_values(
        &self,
        lists: &BTreeMap<String, EntityList>,
    ) -> BTreeMap<String, Vec<String>> {
        lists
            .iter()
            .filter_map(|(name, list)| {
                self.offered(list)
                    .map(|values| (name.clone(), values.to_vec()))
            })
            .collect()
    }

    /// Run one script now and report what it answered.
    ///
    /// The settings page calls this to show the list of a script before a
    /// turn reads it. The preview ignores the cache and the list in
    /// memory, because a reader who asks for the list wants the list of
    /// this moment.
    pub async fn preview(&self, script: &str) -> ScriptPreviewDto {
        let script = script.trim();
        if script.is_empty() {
            return ScriptPreviewDto {
                values: Vec::new(),
                exit_code: None,
                duration_ms: 0,
                error: Some("the entity has no script".to_string()),
            };
        }

        let started = Instant::now();
        match self.runner.run(script, |_, _| {}).await {
            Ok(outcome) => {
                let values = parse_values(&outcome.stdout, self.max_values);
                ScriptPreviewDto {
                    values,
                    exit_code: Some(outcome.exit_code),
                    duration_ms: outcome.duration_ms,
                    error: (outcome.exit_code != 0)
                        .then(|| format!("the script exited with the code {}", outcome.exit_code)),
                }
            }
            Err(err) => ScriptPreviewDto {
                values: Vec::new(),
                exit_code: None,
                duration_ms: started.elapsed().as_millis() as u64,
                error: Some(err.to_string()),
            },
        }
    }

    /// Run one script and read the values it wrote, or why it did not.
    async fn run(&self, script: &str) -> Result<Vec<String>, String> {
        let outcome = self
            .runner
            .run(script, |_, _| {})
            .await
            .map_err(|err| err.to_string())?;
        if outcome.exit_code != 0 {
            let detail = outcome.stderr.trim();
            let detail = if detail.is_empty() {
                String::new()
            } else {
                format!(": {detail}")
            };
            return Err(format!(
                "the script exited with the code {}{detail}",
                outcome.exit_code
            ));
        }
        Ok(parse_values(&outcome.stdout, self.max_values))
    }
}

/// The values of one entity that an engine reads as labels.
///
/// Only a script entity takes its values from a list this way. A closed
/// entity carries its own values and an open entity has none.
pub fn offered_for<'a>(
    entity: &IntentEntityDto,
    offered: &'a BTreeMap<String, Vec<String>>,
) -> &'a [String] {
    if !matches!(entity.kind, EntityKindDto::Script) {
        return &[];
    }
    offered.get(&entity.name).map_or(&[], Vec::as_slice)
}

/// Read the values out of the output of one script.
///
/// One line is one value. A blank line and a value the script wrote twice
/// stay out, and the daemon keeps the first `max` values, so a script
/// that runs away cannot fill the memory of the daemon.
pub fn parse_values(stdout: &str, max: usize) -> Vec<String> {
    let mut values: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let value = line.trim();
        if value.is_empty() || values.iter().any(|kept| kept == value) {
            continue;
        }
        values.push(value.to_string());
        if values.len() == max {
            break;
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::dto::EntityKindDto;
    use chrono::Utc;

    /// Build one script entity for the tests.
    fn entity(id: Uuid, script: &str) -> IntentEntityDto {
        IntentEntityDto {
            id,
            name: "applications".to_string(),
            kind: EntityKindDto::Script,
            values: Vec::new(),
            script: Some(script.to_string()),
            required: true,
        }
    }

    /// Build one intent that carries the given entities.
    fn intent(entities: Vec<IntentEntityDto>) -> IntentDto {
        IntentDto {
            id: Uuid::new_v4(),
            name: "open application".to_string(),
            description: String::new(),
            command: "echo {applications}".to_string(),
            entities,
            examples: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Build a service that runs commands of the tests.
    fn scripts(ttl_secs: u64, max_values: usize, label_budget: usize) -> EntityScripts {
        EntityScripts::new(
            CommandRunner::new(Duration::from_secs(5), 4096),
            Duration::from_secs(ttl_secs),
            max_values,
            label_budget,
        )
    }

    /// A path in a fresh temporary directory.
    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("alice-values-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("the temporary directory is created");
        dir.join(name)
    }

    /// Count the lines of a file, or zero when it does not exist.
    fn lines_of(path: &std::path::Path) -> usize {
        std::fs::read_to_string(path)
            .map(|text| text.lines().count())
            .unwrap_or(0)
    }

    #[test]
    fn parse_values_reads_one_value_per_line() {
        let values = parse_values("firefox\nobs\n\ndiscord \n", 10);
        assert_eq!(values, vec!["firefox", "obs", "discord"]);
    }

    #[test]
    fn parse_values_drops_a_repeated_value() {
        let values = parse_values("obs\nfirefox\nobs\n", 10);
        assert_eq!(values, vec!["obs", "firefox"]);
    }

    #[test]
    fn parse_values_keeps_the_limit() {
        let values = parse_values("one\ntwo\nthree\n", 2);
        assert_eq!(values, vec!["one", "two"]);
    }

    #[test]
    fn parse_values_reads_no_value_out_of_an_empty_output() {
        assert!(parse_values("   \n\n", 10).is_empty());
    }

    #[tokio::test]
    async fn a_script_answers_with_its_values() {
        let service = scripts(60, 10, 24);
        let list = service
            .list(&entity(Uuid::new_v4(), "printf 'firefox\\nobs\\n'"))
            .await;

        assert_eq!(list.values, vec!["firefox", "obs"]);
        assert_eq!(list.error, None);
        assert!(list.is_readable());
    }

    #[tokio::test]
    async fn a_script_that_lists_nothing_reports_a_reason() {
        let service = scripts(60, 10, 24);
        let list = service.list(&entity(Uuid::new_v4(), "true")).await;

        assert!(list.values.is_empty());
        assert!(list.error.is_some());
        assert!(!list.is_readable());
    }

    #[tokio::test]
    async fn a_script_that_fails_reports_its_exit_code() {
        let service = scripts(60, 10, 24);
        let list = service
            .list(&entity(Uuid::new_v4(), "echo broken 1>&2; exit 4"))
            .await;

        let reason = list.error.expect("the script failed");
        assert!(reason.contains('4'), "{reason}");
        assert!(reason.contains("broken"), "{reason}");
    }

    #[tokio::test]
    async fn an_entity_without_a_script_reports_a_reason() {
        let service = scripts(60, 10, 24);
        let mut entity = entity(Uuid::new_v4(), "ls");
        entity.script = None;

        assert!(service.list(&entity).await.error.is_some());
    }

    #[tokio::test]
    async fn a_young_list_answers_without_running_the_script_again() {
        let counter = temp_path("runs");
        let service = scripts(60, 10, 24);
        let entity = entity(
            Uuid::new_v4(),
            &format!("echo x >> {}; echo firefox", counter.display()),
        );

        service.list(&entity).await;
        service.list(&entity).await;

        assert_eq!(lines_of(&counter), 1);
    }

    #[tokio::test]
    async fn an_old_list_runs_the_script_again() {
        let counter = temp_path("runs");
        let service = scripts(0, 10, 24);
        let entity = entity(
            Uuid::new_v4(),
            &format!("echo x >> {}; echo firefox", counter.display()),
        );

        service.list(&entity).await;
        service.list(&entity).await;

        assert_eq!(lines_of(&counter), 2);
    }

    #[tokio::test]
    async fn a_changed_script_runs_again_without_waiting_for_the_ttl() {
        let counter = temp_path("runs");
        let service = scripts(60, 10, 24);
        let id = Uuid::new_v4();
        let script = |name: &str| format!("echo x >> {}; echo {name}", counter.display());

        service.list(&entity(id, &script("firefox"))).await;
        let list = service.list(&entity(id, &script("obs"))).await;

        assert_eq!(list.values, vec!["obs"]);
        assert_eq!(lines_of(&counter), 2);
    }

    #[tokio::test]
    async fn a_broken_script_keeps_the_list_the_daemon_read_last() {
        let service = scripts(0, 10, 24);
        let id = Uuid::new_v4();

        service.list(&entity(id, "echo firefox")).await;
        let list = service.list(&entity(id, "exit 9")).await;

        assert_eq!(list.values, vec!["firefox"]);
        assert!(list.error.is_some());
    }

    #[tokio::test]
    async fn the_lists_of_an_intent_name_their_entity() {
        let service = scripts(60, 10, 24);
        let mut open = entity(Uuid::new_v4(), "echo firefox");
        open.name = "window".to_string();
        open.kind = EntityKindDto::Open;

        let lists = service
            .lists(&intent(vec![entity(Uuid::new_v4(), "echo firefox"), open]))
            .await;

        assert_eq!(lists.len(), 1);
        assert!(lists.contains_key("applications"));
    }

    #[tokio::test]
    async fn a_short_list_is_offered_as_labels() {
        let service = scripts(60, 10, 24);
        let list = service
            .list(&entity(Uuid::new_v4(), "printf 'firefox\\nobs\\n'"))
            .await;

        assert!(matches!(service.offered(&list), Some(values) if values.len() == 2));
        assert!(service.offers_labels(&list));
    }

    #[tokio::test]
    async fn a_long_list_is_not_offered_as_labels() {
        let service = scripts(60, 10, 2);
        let list = service
            .list(&entity(Uuid::new_v4(), "printf 'one\\ntwo\\nthree\\n'"))
            .await;

        assert_eq!(list.values.len(), 3);
        assert!(service.offered(&list).is_none());
        assert!(!service.offers_labels(&list));
    }

    #[tokio::test]
    async fn the_offered_map_holds_the_short_lists_only() {
        let service = scripts(60, 10, 2);
        let lists = service
            .lists(&intent(vec![entity(
                Uuid::new_v4(),
                "printf 'one\ntwo\nthree\n'",
            )]))
            .await;

        assert!(service.offered_values(&lists).is_empty());
    }

    #[tokio::test]
    async fn a_list_without_values_offers_nothing() {
        let service = scripts(60, 10, 24);
        let list = service.list(&entity(Uuid::new_v4(), "true")).await;

        assert!(service.offered(&list).is_none());
    }

    #[tokio::test]
    async fn a_preview_reports_the_values_of_the_moment() {
        let service = scripts(60, 10, 24);
        let preview = service.preview("printf 'firefox\\nobs\\n'").await;

        assert_eq!(preview.values, vec!["firefox", "obs"]);
        assert_eq!(preview.exit_code, Some(0));
        assert!(preview.error.is_none());
    }

    #[tokio::test]
    async fn a_preview_reports_a_failing_script() {
        let service = scripts(60, 10, 24);
        let preview = service.preview("exit 3").await;

        assert_eq!(preview.exit_code, Some(3));
        assert!(preview.error.is_some());
    }

    #[tokio::test]
    async fn a_preview_without_a_script_reports_a_reason() {
        let service = scripts(60, 10, 24);
        assert!(service.preview("   ").await.error.is_some());
    }

    #[tokio::test]
    async fn a_preview_ignores_the_list_in_memory() {
        let counter = temp_path("runs");
        let service = scripts(60, 10, 24);
        let script = format!("echo x >> {}; echo firefox", counter.display());

        service.list(&entity(Uuid::new_v4(), &script)).await;
        service.preview(&script).await;

        assert_eq!(lines_of(&counter), 2);
    }
}
