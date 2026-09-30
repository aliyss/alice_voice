//! Facts of the memory.
//!
//! A fact is one relation of one concept. The write is dated: a new value
//! closes the older one rather than deleting it, and a fact the memory
//! already holds is not a repeat to discard but a second sighting that
//! raises its confidence. The module also settles which concept a name
//! belongs to, so a reader that names the same thing twice teaches one
//! concept rather than two, and it can merge two concepts a reader opened
//! apart.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveValue::Set, ColumnTrait, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

use alice_core::dto::MemorySavedFactDto;

use crate::librarian::entity::{memory_alias, memory_edge, memory_node};
use crate::librarian::extract::ExtractedFact;
use crate::librarian::gate::{self, Verdict};
use crate::librarian::store::LibrarianStore;

/// How sure the memory is of a fact the reader rated nothing.
pub const DEFAULT_CONFIDENCE: f32 = 0.6;

/// How much a fact the reader rated nothing is worth.
pub const DEFAULT_IMPORTANCE: f32 = 0.5;

/// How much one more sighting of a fact raises its confidence.
const CONFIRMATION_STEP: f32 = 0.1;

/// How many concepts the memory reads while it settles a name.
///
/// The read is a scan, so it stops at a number of concepts a personal
/// memory reaches only after years of turns.
const RESOLUTION_LIMIT: u64 = 500;

/// One fact about to be written.
#[derive(Clone, Copy, Debug)]
pub struct FactWrite<'a> {
    /// The relation the fact names, for example `lives_in`.
    pub relation: &'a str,
    /// The value the fact carries.
    pub value: &'a str,
    /// How sure the reader was of the fact, from zero to one.
    pub confidence: f32,
    /// How much the fact is worth, from zero to one.
    pub importance: f32,
    /// The episode the reader read the fact from, or null.
    pub episode: Option<Uuid>,
}

/// What one write of a fact did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FactChange {
    /// The edge that now carries the value.
    pub id: Uuid,
    /// Whether the write closed an older value.
    pub changed: bool,
    /// Whether the write confirmed a fact the memory already held.
    pub confirmed: bool,
}

impl LibrarianStore {
    /// Write one fact and close the older value of the same relation.
    ///
    /// A write of a fact the memory already holds is not a repeat to throw
    /// away: it is the second time the memory saw it, so the write counts
    /// the sighting, raises the confidence, and moves the time the memory
    /// last saw it. The value stays as the reader first spelled it, so the
    /// memory shows one spelling of one fact.
    pub async fn add_fact(
        &self,
        subject: Uuid,
        fact: FactWrite<'_>,
        at: DateTime<Utc>,
    ) -> Result<FactChange, DbErr> {
        let relation = relation_key(fact.relation);
        let current: Vec<memory_edge::Model> = self
            .current_edges(subject)
            .await?
            .into_iter()
            .filter(|edge| relation_key(&edge.relation) == relation)
            .collect();
        if let Some(same) = current
            .iter()
            .find(|edge| same_fact_value(&edge.value, fact.value))
        {
            let mut active: memory_edge::ActiveModel = same.clone().into();
            active.confirmations = Set(same.confirmations + 1);
            active.confidence = Set((same.confidence + CONFIRMATION_STEP).min(1.0));
            active.importance = Set(same.importance.max(fact.importance.clamp(0.0, 1.0)));
            active.last_confirmed_at = Set(at.fixed_offset());
            memory_edge::Entity::update(active).exec(self.db()).await?;
            return Ok(FactChange {
                id: same.id,
                changed: false,
                confirmed: true,
            });
        }
        let id = Uuid::new_v4();
        for edge in current {
            let mut active: memory_edge::ActiveModel = edge.into();
            active.invalid_at = Set(Some(at.fixed_offset()));
            active.superseded_by = Set(Some(id));
            memory_edge::Entity::update(active).exec(self.db()).await?;
        }
        memory_edge::Entity::insert(memory_edge::ActiveModel {
            id: Set(id),
            subject_id: Set(subject),
            relation: Set(relation),
            value: Set(fact.value.to_string()),
            confidence: Set(fact.confidence.clamp(0.0, 1.0)),
            importance: Set(fact.importance.clamp(0.0, 1.0)),
            confirmations: Set(0),
            last_confirmed_at: Set(at.fixed_offset()),
            valid_at: Set(at.fixed_offset()),
            invalid_at: Set(None),
            superseded_by: Set(None),
            source_episode: Set(fact.episode),
        })
        .exec_without_returning(self.db())
        .await?;
        Ok(FactChange {
            id,
            changed: true,
            confirmed: false,
        })
    }

    /// Read the concept a name belongs to.
    ///
    /// The reader is a model, so it names the same thing `user` one turn
    /// and `flurin` the next. The memory settles the name before it opens
    /// a concept: the key it holds, a name an earlier turn used for that
    /// concept, a key or a name that says the same thing without its
    /// punctuation, and the name the memory shows for a concept all reach
    /// one node. A name that reaches nothing opens a new concept.
    pub async fn resolve_subject(&self, key: &str, title: &str) -> Result<Option<Uuid>, DbErr> {
        if let Some(node) = self.find_node_by_key(&relation_key(key)).await? {
            return Ok(Some(node.id));
        }
        let wanted = same_name(key);
        if wanted.is_empty() {
            return Ok(None);
        }
        if let Some(owner) = self.alias_owner(&wanted).await? {
            return Ok(Some(owner));
        }
        let wanted_title = same_name(title);
        for node in self.list_nodes(RESOLUTION_LIMIT).await? {
            if same_name(&node.key) == wanted {
                return Ok(Some(node.id));
            }
            if !wanted_title.is_empty()
                && (same_name(&node.title) == wanted_title || same_name(&node.key) == wanted_title)
            {
                return Ok(Some(node.id));
            }
            if same_name(&node.title) == wanted {
                return Ok(Some(node.id));
            }
        }
        Ok(None)
    }

    /// Keep one name for one concept.
    ///
    /// A name the memory already holds for another concept is left alone.
    /// Only a reader settles which concept a name belongs to, and a merge
    /// answers that question through its own call.
    pub async fn record_alias(&self, node_id: Uuid, name: &str) -> Result<(), DbErr> {
        let alias = same_name(name);
        if alias.is_empty() {
            return Ok(());
        }
        if memory_alias::Entity::find_by_id(alias.clone())
            .one(self.db())
            .await?
            .is_some()
        {
            return Ok(());
        }
        memory_alias::Entity::insert(memory_alias::ActiveModel {
            alias: Set(alias),
            node_id: Set(node_id),
        })
        .exec_without_returning(self.db())
        .await?;
        Ok(())
    }

    /// Read the node one known name belongs to.
    async fn alias_owner(&self, alias: &str) -> Result<Option<Uuid>, DbErr> {
        Ok(memory_alias::Entity::find_by_id(alias.to_string())
            .one(self.db())
            .await?
            .map(|row| row.node_id))
    }

    /// Move every fact and name of one concept onto another.
    ///
    /// The merge is for the concepts a reader opened apart, so the target
    /// keeps its name and its facts. A relation the two concepts both
    /// hold settles by recency: the newer value stays current and the
    /// older one reports what it replaced, exactly as if a later turn had
    /// taught the relation again. A relation only the source holds moves
    /// as it stands, history included. The key of the source becomes a
    /// name of the target, so a later turn that uses it reaches the target
    /// instead of opening the concept again.
    pub async fn merge_nodes(
        &self,
        source: Uuid,
        target: Uuid,
        at: DateTime<Utc>,
    ) -> Result<bool, DbErr> {
        if source == target {
            return Ok(false);
        }
        let Some(source_node) = memory_node::Entity::find_by_id(source)
            .one(self.db())
            .await?
        else {
            return Ok(false);
        };
        if memory_node::Entity::find_by_id(target)
            .one(self.db())
            .await?
            .is_none()
        {
            return Ok(false);
        }

        // 1. Settle the relations the two concepts share.
        let target_edges = self.current_edges(target).await?;
        for edge in self.current_edges(source).await? {
            let relation = relation_key(&edge.relation);
            let Some(clash) = target_edges
                .iter()
                .find(|other| relation_key(&other.relation) == relation)
            else {
                continue;
            };
            // One fact twice points at the fact the target already holds
            // rather than reading as a second value, and two different
            // values settle by recency.
            let (loser, winner) =
                if same_fact_value(&clash.value, &edge.value) || clash.valid_at >= edge.valid_at {
                    (edge.clone(), clash.clone())
                } else {
                    (clash.clone(), edge.clone())
                };
            let mut active: memory_edge::ActiveModel = loser.into();
            active.invalid_at = Set(Some(at.fixed_offset()));
            active.superseded_by = Set(Some(winner.id));
            memory_edge::Entity::update(active).exec(self.db()).await?;
        }

        // 2. Move what is left, keep the name, and drop the empty concept.
        for edge in memory_edge::Entity::find()
            .filter(memory_edge::Column::SubjectId.eq(source))
            .all(self.db())
            .await?
        {
            let mut active: memory_edge::ActiveModel = edge.into();
            active.subject_id = Set(target);
            memory_edge::Entity::update(active).exec(self.db()).await?;
        }
        for alias in memory_alias::Entity::find()
            .filter(memory_alias::Column::NodeId.eq(source))
            .all(self.db())
            .await?
        {
            let mut active: memory_alias::ActiveModel = alias.into();
            active.node_id = Set(target);
            memory_alias::Entity::update(active).exec(self.db()).await?;
        }
        self.record_alias(target, &source_node.key).await?;
        self.record_alias(target, &source_node.title).await?;
        self.delete_node_row(source).await?;
        Ok(true)
    }

    /// Write every fact one reading of a turn taught.
    ///
    /// The gate reads each fact before it reaches the memory, so a
    /// credential or a sentence never becomes a memory the daemon repeats
    /// in a later prompt. Each fact lands on the concept the memory
    /// already holds for its subject, or opens one. The answer carries the
    /// facts the write really changed, so the transcript marks the turns
    /// that taught something new and stays quiet about a turn that only
    /// said again what the memory held.
    pub async fn apply_facts(
        &self,
        episode: Uuid,
        facts: &[ExtractedFact],
        at: DateTime<Utc>,
    ) -> Result<Vec<MemorySavedFactDto>, DbErr> {
        let mut saved: Vec<MemorySavedFactDto> = Vec::new();
        for fact in facts {
            if let Verdict::Drop(reason) = gate::judge(fact) {
                tracing::debug!(
                    reason,
                    subject = %fact.subject,
                    relation = %fact.relation,
                    "the memory left one fact out"
                );
                continue;
            }
            let node = match self.resolve_subject(&fact.subject, &fact.title).await? {
                Some(id) => {
                    // The reader named a concept the memory already holds
                    // under another name, so the name is kept for the turns
                    // that use it again.
                    self.record_alias(id, &fact.subject).await?;
                    self.record_alias(id, &fact.title).await?;
                    self.touch_node(id, at).await?;
                    id
                }
                None => self.upsert_node(&fact.subject, &fact.title, "", at).await?,
            };
            let change = self
                .add_fact(
                    node,
                    FactWrite {
                        relation: &fact.relation,
                        value: &fact.value,
                        confidence: fact.confidence,
                        importance: fact.importance,
                        episode: Some(episode),
                    },
                    at,
                )
                .await?;
            if change.changed {
                saved.push(MemorySavedFactDto {
                    concept: fact.title.clone(),
                    relation: fact.relation.clone(),
                    value: fact.value.clone(),
                });
            }
        }
        Ok(saved)
    }
}

/// The shape the memory writes a relation in.
///
/// A relation names a kind of fact rather than a sentence about one, so
/// the memory holds one spelling of it: lower case, with the words joined
/// by one underscore. A reader that writes `lives in`, `Lives In`, or
/// `lives-in` then teaches one relation instead of three, and the shape is
/// read back through the relations the memory already holds, so a row an
/// older write left behind is closed rather than kept beside the new one.
pub(crate) fn relation_key(relation: &str) -> String {
    relation
        .to_lowercase()
        .split(|ch: char| ch == '_' || ch == '-' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

/// Whether two values of a fact say the same thing.
///
/// A value is written by a model, so the same fact comes back as
/// `Singapore` one turn and `singapore ` the next. A repeated fact is one
/// fact, and a memory that stores every spelling of it grows with every
/// turn that says it again, so the values are read without regard to case
/// or to the room between their words.
pub(crate) fn same_fact_value(left: &str, right: &str) -> bool {
    let room = |value: &str| {
        value
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    room(left) == room(right)
}

/// The shape two names share when they name the same thing.
///
/// The punctuation of a name is what a model varies, so a name is read as
/// its letters and digits alone: `project_alice`, `project alice`, and
/// `Project Alice` are one name.
pub(crate) fn same_name(name: &str) -> String {
    name.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db;

    /// Build a store on an in memory database with the librarian schema.
    async fn store() -> LibrarianStore {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("an in memory database answers");
        db::init_schema(&db).await.expect("the schema is created");
        LibrarianStore::new(db)
    }

    /// Build one fact the way the reader answers it.
    fn fact(subject: &str, title: &str, relation: &str, value: &str) -> ExtractedFact {
        ExtractedFact {
            subject: subject.to_string(),
            title: title.to_string(),
            relation: relation.to_string(),
            value: value.to_string(),
            confidence: DEFAULT_CONFIDENCE,
            importance: DEFAULT_IMPORTANCE,
        }
    }

    /// Write one fact with the default rating.
    async fn write(
        store: &LibrarianStore,
        subject: Uuid,
        relation: &str,
        value: &str,
    ) -> FactChange {
        store
            .add_fact(
                subject,
                FactWrite {
                    relation,
                    value,
                    confidence: DEFAULT_CONFIDENCE,
                    importance: DEFAULT_IMPORTANCE,
                    episode: None,
                },
                Utc::now(),
            )
            .await
            .expect("the fact is written")
    }

    #[tokio::test]
    async fn the_same_fact_twice_keeps_one_edge() {
        let store = store().await;
        let node = store
            .upsert_node("flurin", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "lives_in", "Singapore").await;
        let second = write(&store, node, "lives_in", "Singapore").await;

        let facts = store.facts_of(node).await.expect("the facts are read");
        assert_eq!(facts.len(), 1);
        assert!(facts[0].current);
        assert!(second.confirmed);
        assert!(!second.changed);
    }

    #[tokio::test]
    async fn a_repeated_fact_raises_its_confidence() {
        let store = store().await;
        let node = store
            .upsert_node("flurin", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "lives_in", "Singapore").await;
        write(&store, node, "lives_in", "singapore").await;

        let facts = store.facts_of(node).await.expect("the facts are read");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].confirmations, 1);
        assert!(facts[0].confidence > DEFAULT_CONFIDENCE);
        assert!(facts[0].last_confirmed_at >= facts[0].valid_at);
    }

    #[tokio::test]
    async fn a_fact_the_gate_drops_never_reaches_the_memory() {
        let store = store().await;
        let episode = Uuid::new_v4();
        let facts = vec![
            fact("user", "User", "lives_in", "Zurich"),
            fact(
                "user",
                "User",
                "api_key",
                "sk-3f9a2c1b4d5e6f708192a3b4c5d6e7f8",
            ),
        ];

        let saved = store
            .apply_facts(episode, &facts, Utc::now())
            .await
            .expect("the facts are written");

        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].relation, "lives_in");
        let nodes = store.query("user", 10).await.expect("the memory is read");
        assert_eq!(nodes[0].facts.len(), 1);
    }

    #[tokio::test]
    async fn a_second_name_for_one_thing_lands_on_one_concept() {
        let store = store().await;
        let at = Utc::now();
        // The first turn opens the concept the reader named `user`.
        store
            .apply_facts(
                Uuid::new_v4(),
                &[fact("user", "Flurin", "lives_in", "Singapore")],
                at,
            )
            .await
            .expect("the first reading is written");
        // The second turn names the same person `flurin`.
        store
            .apply_facts(
                Uuid::new_v4(),
                &[fact("flurin", "Flurin", "prefers", "Neovim")],
                at,
            )
            .await
            .expect("the second reading is written");

        let nodes = store.query("flurin", 10).await.expect("the memory is read");
        assert_eq!(nodes.len(), 1, "one concept holds both turns");
        assert_eq!(nodes[0].facts.len(), 2);
    }

    #[tokio::test]
    async fn a_name_the_memory_learned_keeps_its_concept() {
        let store = store().await;
        let at = Utc::now();
        store
            .apply_facts(
                Uuid::new_v4(),
                &[fact("user", "Flurin", "lives_in", "Singapore")],
                at,
            )
            .await
            .expect("the first reading is written");
        // A name that shares no word with the concept still reaches it
        // once a turn has taught it.
        store
            .apply_facts(
                Uuid::new_v4(),
                &[fact("flurin", "Flurin", "uses", "NixOS")],
                at,
            )
            .await
            .expect("the second reading is written");
        store
            .apply_facts(
                Uuid::new_v4(),
                &[fact("flurin", "Flurin", "prefers", "Neovim")],
                at,
            )
            .await
            .expect("the third reading is written");

        let nodes = store.query("user", 10).await.expect("the memory is read");
        assert_eq!(nodes.len(), 1, "one concept holds every turn");
        assert_eq!(nodes[0].facts.len(), 3);
    }

    #[tokio::test]
    async fn a_merge_keeps_the_newest_value_of_a_shared_relation() {
        let store = store().await;
        let at = Utc::now();
        let user = store
            .upsert_node("user", "Flurin", "", at)
            .await
            .expect("the node is stored");
        write(&store, user, "lives_in", "Singapore").await;
        let flurin = store
            .upsert_node("flurin", "Flurin", "", at)
            .await
            .expect("the second node is stored");
        write(&store, flurin, "lives_in", "Berlin").await;
        write(&store, flurin, "prefers", "Neovim").await;

        let merged = store
            .merge_nodes(flurin, user, at)
            .await
            .expect("the merge applies");

        assert!(merged);
        let node = store
            .find_node(user)
            .await
            .expect("the memory is read")
            .expect("the concept is there");
        assert_eq!(node.facts.len(), 3, "the history of both concepts stays");
        let mut current = node
            .facts
            .iter()
            .filter(|fact| fact.current)
            .map(|fact| format!("{}={}", fact.relation, fact.value))
            .collect::<Vec<_>>();
        current.sort();
        assert_eq!(
            current,
            vec!["lives_in=Berlin".to_string(), "prefers=Neovim".to_string()]
        );
        assert!(store
            .find_node(flurin)
            .await
            .expect("the memory is read")
            .is_none());
    }

    #[tokio::test]
    async fn a_merge_keeps_the_name_of_the_concept_it_absorbed() {
        let store = store().await;
        let at = Utc::now();
        let user = store
            .upsert_node("user", "Flurin", "", at)
            .await
            .expect("the node is stored");
        let flurin = store
            .upsert_node("flurin", "Flurin", "", at)
            .await
            .expect("the second node is stored");
        store
            .merge_nodes(flurin, user, at)
            .await
            .expect("the merge applies");

        assert_eq!(
            store
                .resolve_subject("flurin", "Flurin")
                .await
                .expect("the name is settled"),
            Some(user)
        );
    }

    #[tokio::test]
    async fn a_name_that_reaches_nothing_opens_a_concept() {
        let store = store().await;
        let user = store
            .upsert_node("user", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");

        assert_eq!(
            store
                .resolve_subject("machine", "Machine")
                .await
                .expect("the name is settled"),
            None
        );
        assert_ne!(
            store
                .resolve_subject("user", "Flurin")
                .await
                .expect("the name is settled"),
            Some(Uuid::new_v4())
        );
        assert_eq!(
            store
                .resolve_subject("USER", "Flurin")
                .await
                .expect("the name is settled"),
            Some(user)
        );
    }

    #[tokio::test]
    async fn a_relation_of_another_shape_is_the_same_relation() {
        let store = store().await;
        let node = store
            .upsert_node("flurin", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "lives in", "Singapore").await;
        write(&store, node, "Lives_In", "singapore").await;

        let facts = store.facts_of(node).await.expect("the facts are read");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].relation, "lives_in");
        assert_eq!(facts[0].value, "Singapore");
    }

    #[tokio::test]
    async fn a_new_value_closes_a_relation_of_another_shape() {
        let store = store().await;
        let node = store
            .upsert_node("flurin", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "lives in", "Singapore").await;
        write(&store, node, "lives_in", "Berlin").await;

        let facts = store.facts_of(node).await.expect("the facts are read");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts.iter().filter(|fact| fact.current).count(), 1);
        assert_eq!(
            facts
                .iter()
                .find(|fact| fact.current)
                .map(|fact| fact.value.as_str()),
            Some("Berlin")
        );
    }
}
