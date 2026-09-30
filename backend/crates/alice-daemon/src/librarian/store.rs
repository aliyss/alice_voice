//! Memory store of the librarian.
//!
//! The store keeps one node per concept and one dated edge per relation.
//! A new value closes the older edge with the time it stopped being true
//! and never deletes it, so the memory answers a question about the past
//! as well as one about now.
//!
//! The read path is ranked rather than flat: the seed of a turn shows the
//! concepts the turn is about first, the certainty of a fact lifts it, and
//! the memory says how old a fact is, so the model can question one that
//! nothing confirmed for a while.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use sea_orm::sea_query::{Expr, ExprTrait, Func, SimpleExpr, Value};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use uuid::Uuid;

use alice_core::dto::{LibrarianLintDto, MemoryFactDto, MemoryNodeDto};

use crate::librarian::entity::{memory_edge, memory_episode, memory_node};
use crate::librarian::facts::same_name;

/// The line that opens the seed of a turn.
const SEED_HEADER: &str = "Concepts the daemon already remembers about this user:";

/// How many facts of one concept the seed of a turn shows.
///
/// A concept of many facts would otherwise fill the prompt alone, and the
/// seed is a hint about what the memory holds rather than the memory.
const MAX_SEED_FACTS: usize = 8;

/// The days after which a fact the seed shows reads as old.
const STALE_AFTER_DAYS: i64 = 30;

/// What a concept the turn names adds to its rank.
const NAMED_SCORE: f64 = 1.0;

/// How many words of a turn name a concept in full.
///
/// A question holds filler, so a concept the turn names with three of its
/// words counts as named, and one it names with a single word counts as a
/// third of that.
const FULLY_NAMED_TERMS: usize = 3;

/// The shortest word of a turn the memory looks for.
const MIN_TERM_CHARS: usize = 3;

/// The most words of one turn the memory looks for at once.
const MAX_SEARCH_TERMS: usize = 12;

/// The most nodes one search reads before it ranks them.
const SEARCH_CANDIDATES: u64 = 200;

/// The words of a turn that name nothing on their own.
const STOP_WORDS: &[&str] = &[
    "and", "the", "for", "with", "what", "does", "did", "how", "when", "where", "which", "who",
    "why", "are", "was", "were", "you", "your", "mine", "this", "that", "there", "have", "has",
    "can", "could", "would", "should", "tell", "know", "about", "again", "please", "from", "into",
];

/// The most the freshness of a concept adds to its rank.
const FRESHNESS_SCORE: f64 = 0.3;

/// The days over which the freshness of a concept halves.
const FRESHNESS_HALF_LIFE_DAYS: f64 = 30.0;

/// The most the certainty of a concept adds to its rank.
const CERTAINTY_SCORE: f64 = 0.5;

/// How much one sighting of a fact raises its weight.
const CONFIRMATION_WEIGHT: f64 = 0.1;

/// Store of the long term memory of the daemon.
#[derive(Clone, Debug)]
pub struct LibrarianStore {
    db: DatabaseConnection,
}

impl LibrarianStore {
    /// Create a new memory store.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Borrow the database connection.
    pub(crate) fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Number of stored memory nodes.
    pub async fn count_nodes(&self) -> Result<u64, DbErr> {
        memory_node::Entity::find().count(&self.db).await
    }

    /// Number of stored memory edges, closed edges included.
    pub async fn count_edges(&self) -> Result<u64, DbErr> {
        memory_edge::Entity::find().count(&self.db).await
    }

    /// Number of stored edges that are true now.
    ///
    /// The count beside [`count_edges`](Self::count_edges) tells a reader
    /// how much of the memory describes now and how much of it is the
    /// history of what it used to hold.
    pub async fn count_current_edges(&self) -> Result<u64, DbErr> {
        memory_edge::Entity::find()
            .filter(memory_edge::Column::InvalidAt.is_null())
            .count(&self.db)
            .await
    }

    /// Number of stored edges a later turn taught again.
    pub async fn count_confirmed_edges(&self) -> Result<u64, DbErr> {
        memory_edge::Entity::find()
            .filter(memory_edge::Column::Confirmations.gt(0))
            .count(&self.db)
            .await
    }

    /// Read the most recently changed nodes.
    pub async fn list_nodes(&self, limit: u64) -> Result<Vec<memory_node::Model>, DbErr> {
        memory_node::Entity::find()
            .order_by_desc(memory_node::Column::UpdatedAt)
            .limit(limit)
            .all(&self.db)
            .await
    }

    /// Find one node by its key.
    pub async fn find_node_by_key(&self, key: &str) -> Result<Option<memory_node::Model>, DbErr> {
        memory_node::Entity::find()
            .filter(memory_node::Column::Key.eq(key))
            .one(&self.db)
            .await
    }

    /// Read one node and its facts by its identifier.
    pub async fn find_node(&self, id: Uuid) -> Result<Option<MemoryNodeDto>, DbErr> {
        let Some(node) = memory_node::Entity::find_by_id(id).one(&self.db).await? else {
            return Ok(None);
        };
        let facts = self.facts_of(id).await?;
        Ok(Some(node_of(node, facts)))
    }

    /// Retire one fact now and keep the fact it replaced.
    ///
    /// The fact is closed rather than deleted, so the memory still answers
    /// a question about the past. The call returns the identifier of the
    /// concept the fact belongs to, or null when no fact carries that
    /// identifier.
    pub async fn retire_fact(
        &self,
        fact_id: Uuid,
        at: DateTime<Utc>,
    ) -> Result<Option<Uuid>, DbErr> {
        let Some(edge) = memory_edge::Entity::find_by_id(fact_id)
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        if edge.invalid_at.is_none() {
            let mut active: memory_edge::ActiveModel = edge.clone().into();
            active.invalid_at = Set(Some(at.fixed_offset()));
            memory_edge::Entity::update(active).exec(&self.db).await?;
        }
        Ok(Some(edge.subject_id))
    }

    /// Delete one concept and every fact of it.
    ///
    /// Returns false when no concept carries that identifier, so the caller
    /// can answer a not found rather than an empty success.
    pub async fn delete_node(&self, id: Uuid) -> Result<bool, DbErr> {
        let deleted = memory_node::Entity::delete_by_id(id).exec(&self.db).await?;
        if deleted.rows_affected == 0 {
            return Ok(false);
        }
        memory_edge::Entity::delete_many()
            .filter(memory_edge::Column::SubjectId.eq(id))
            .exec(&self.db)
            .await?;
        Ok(true)
    }

    /// Delete one concept and leave its facts where they are.
    ///
    /// The merge moves the facts of one concept onto another and then
    /// drops the concept it emptied, so this call must not reach them.
    pub(crate) async fn delete_node_row(&self, id: Uuid) -> Result<(), DbErr> {
        memory_node::Entity::delete_by_id(id).exec(&self.db).await?;
        Ok(())
    }

    /// Move the time one concept last changed.
    ///
    /// A turn that teaches an existing concept changes it as much as a
    /// turn that opens one, so the concept reads as recently changed in
    /// both cases and the seed of a later turn weighs it the same way.
    pub(crate) async fn touch_node(&self, id: Uuid, at: DateTime<Utc>) -> Result<(), DbErr> {
        let active = memory_node::ActiveModel {
            id: Set(id),
            updated_at: Set(at.fixed_offset()),
            ..Default::default()
        };
        memory_node::Entity::update(active).exec(&self.db).await?;
        Ok(())
    }

    /// Create a node or enrich the node of that key.
    ///
    /// An existing node keeps the values the write does not name, so a
    /// turn that learned one new fact never erases the rest of a concept.
    pub async fn upsert_node(
        &self,
        key: &str,
        title: &str,
        body: &str,
        at: DateTime<Utc>,
    ) -> Result<Uuid, DbErr> {
        let stamp = at.fixed_offset();
        if let Some(existing) = self.find_node_by_key(key).await? {
            let title = if title.trim().is_empty() {
                existing.title.clone()
            } else {
                title.to_string()
            };
            let body = if body.trim().is_empty() {
                existing.body.clone()
            } else {
                body.to_string()
            };
            let mut active: memory_node::ActiveModel = existing.clone().into();
            active.title = Set(title);
            active.body = Set(body);
            active.updated_at = Set(stamp);
            memory_node::Entity::update(active).exec(&self.db).await?;
            return Ok(existing.id);
        }
        let id = Uuid::new_v4();
        memory_node::Entity::insert(memory_node::ActiveModel {
            id: Set(id),
            key: Set(key.to_string()),
            title: Set(title.to_string()),
            body: Set(body.to_string()),
            created_at: Set(stamp),
            updated_at: Set(stamp),
        })
        .exec_without_returning(&self.db)
        .await?;
        Ok(id)
    }

    /// Write the title and the body of one node verbatim.
    ///
    /// The ingestion path keeps what a partial write does not name, but a
    /// reader who edits a concept owns what it says: an emptied body has
    /// to clear it. The manual write calls this after the upsert, so the
    /// editor is authoritative and the model is not.
    pub async fn set_node_content(
        &self,
        id: Uuid,
        title: &str,
        body: &str,
        at: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let active = memory_node::ActiveModel {
            id: Set(id),
            title: Set(title.to_string()),
            body: Set(body.to_string()),
            updated_at: Set(at.fixed_offset()),
            ..Default::default()
        };
        memory_node::Entity::update(active).exec(&self.db).await?;
        Ok(())
    }

    /// Read the facts of one node, current first.
    pub async fn facts_of(&self, subject: Uuid) -> Result<Vec<MemoryFactDto>, DbErr> {
        Ok(self
            .facts_of_many(&[subject])
            .await?
            .remove(&subject)
            .unwrap_or_default())
    }

    /// Read the facts of several nodes at once.
    ///
    /// One read answers for the whole page. A read per node would ask the
    /// database once per concept, which costs more the more the memory
    /// holds and the more of it one turn shows.
    async fn facts_of_many(
        &self,
        subjects: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<MemoryFactDto>>, DbErr> {
        if subjects.is_empty() {
            return Ok(HashMap::new());
        }
        let edges = memory_edge::Entity::find()
            .filter(memory_edge::Column::SubjectId.is_in(subjects.to_vec()))
            .order_by_desc(memory_edge::Column::ValidAt)
            .all(&self.db)
            .await?;
        let mut facts: HashMap<Uuid, Vec<MemoryFactDto>> = HashMap::new();
        for edge in edges {
            facts
                .entry(edge.subject_id)
                .or_default()
                .push(fact_of(edge));
        }
        for list in facts.values_mut() {
            sort_facts(list);
        }
        Ok(facts)
    }

    /// Search the memory for the nodes a text names.
    ///
    /// An empty text returns the most recently changed nodes. A text
    /// matches the key, the title, the body, the relation, or the value,
    /// so a question about a value finds the concept that carries it.
    pub async fn query(&self, text: &str, limit: u64) -> Result<Vec<MemoryNodeDto>, DbErr> {
        let needle = text.trim();
        if needle.is_empty() {
            return self.recent_nodes(limit).await;
        }
        let mut nodes = self
            .named_nodes(needle, limit)
            .await?
            .into_iter()
            .map(|(_, node)| node)
            .collect::<Vec<_>>();
        nodes.truncate(limit as usize);
        self.with_facts(nodes).await
    }

    /// Read the nodes one text names, the best named first.
    ///
    /// A turn is a question rather than a name, so the read looks at the
    /// words of it: a question about the editor of the user finds a
    /// concept called `editor` and one whose value says `Neovim`. The
    /// words that name nothing on their own are left out, and a concept
    /// the text names with more of its words comes first, so the concept
    /// the question is about outranks one it brushes against.
    async fn named_nodes(
        &self,
        needle: &str,
        limit: u64,
    ) -> Result<Vec<(usize, memory_node::Model)>, DbErr> {
        let terms = search_terms(needle);
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let mut by_node = Condition::any();
        let mut by_edge = Condition::any();
        for term in &terms {
            by_node = by_node
                .add(lower_like(memory_node::Column::Key, term))
                .add(lower_like(memory_node::Column::Title, term))
                .add(lower_like(memory_node::Column::Body, term));
            by_edge = by_edge
                .add(lower_like(memory_edge::Column::Relation, term))
                .add(lower_like(memory_edge::Column::Value, term));
        }
        let nodes = memory_node::Entity::find()
            .filter(by_node)
            .order_by_desc(memory_node::Column::UpdatedAt)
            .limit(limit.max(SEARCH_CANDIDATES))
            .all(&self.db)
            .await?;
        let edges = memory_edge::Entity::find()
            .filter(by_edge)
            .all(&self.db)
            .await?;

        let mut ranked = nodes
            .into_iter()
            .map(|node| (named_terms(&terms, &node), node))
            .filter(|(named, _)| *named > 0)
            .collect::<Vec<_>>();
        let mut seen = ranked
            .iter()
            .map(|(_, node)| node.id)
            .collect::<HashSet<Uuid>>();
        for edge in edges {
            if !seen.insert(edge.subject_id) {
                continue;
            }
            if let Some(node) = memory_node::Entity::find_by_id(edge.subject_id)
                .one(&self.db)
                .await?
            {
                ranked.push((1, node));
            }
        }
        ranked.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then(right.1.updated_at.cmp(&left.1.updated_at))
        });
        Ok(ranked)
    }

    /// Read the nodes a seed of a turn carries, as plain text.
    ///
    /// The seed is the whole of what one turn is shown of the memory, so
    /// the concepts the turn is about come first: a turn about the editor
    /// has to see what the memory holds about the editor, not the newest
    /// concept of the day. A turn that names nothing falls back on the
    /// concepts the memory changed last, so the model still learns that a
    /// memory exists and what it holds.
    ///
    /// Three signals rank a concept: whether the turn names it, how
    /// recently the memory changed it, and how certain its best fact is.
    /// The rank is a number rather than a rule, so a fact the user stated
    /// twice outranks a one-off detail the assistant mentioned.
    pub async fn seed(&self, text: &str, limit: usize) -> Result<String, DbErr> {
        if limit == 0 {
            return Ok(String::new());
        }
        let needle = text.trim();
        let mut candidates: Vec<(usize, memory_node::Model)> = Vec::new();
        let mut seen: HashSet<Uuid> = HashSet::new();
        if !needle.is_empty() {
            for (named, node) in self.named_nodes(needle, limit as u64 * 2).await? {
                if seen.insert(node.id) {
                    candidates.push((named, node));
                }
            }
        }
        for node in self.list_nodes(limit as u64).await? {
            if seen.insert(node.id) {
                candidates.push((0, node));
            }
        }
        if candidates.is_empty() {
            return Ok(String::new());
        }

        let ids = candidates
            .iter()
            .map(|(_, node)| node.id)
            .collect::<Vec<_>>();
        let facts = self.facts_of_many(&ids).await?;
        let now = Utc::now();
        let mut ranked: Vec<(f64, memory_node::Model, Vec<MemoryFactDto>)> = candidates
            .into_iter()
            .map(|(named, node)| {
                let facts = facts.get(&node.id).cloned().unwrap_or_default();
                let score = seed_score(named, &node, &facts, now);
                (score, node, facts)
            })
            .collect();
        ranked.sort_by(|left, right| right.0.partial_cmp(&left.0).unwrap_or(Ordering::Equal));
        ranked.truncate(limit);

        let mut lines = vec![SEED_HEADER.to_string()];
        for (_, node, facts) in ranked {
            lines.push(seed_line(&node, &facts, now));
        }
        Ok(lines.join("\n"))
    }

    /// Report the concepts two keys name, the nodes no relation reaches,
    /// and the closed relations.
    pub async fn lint(&self, limit: u64) -> Result<LibrarianLintDto, DbErr> {
        let nodes = self.list_nodes(limit).await?;
        let mut orphans: Vec<memory_node::Model> = Vec::new();
        for node in &nodes {
            let facts = memory_edge::Entity::find()
                .filter(memory_edge::Column::SubjectId.eq(node.id))
                .count(&self.db)
                .await?;
            if facts == 0 {
                orphans.push(node.clone());
            }
        }
        let closed_edges = memory_edge::Entity::find()
            .filter(memory_edge::Column::InvalidAt.is_not_null())
            .order_by_desc(memory_edge::Column::InvalidAt)
            .limit(limit)
            .all(&self.db)
            .await?;
        Ok(LibrarianLintDto {
            duplicates: self.with_facts(duplicate_nodes(&nodes)).await?,
            orphans: self.with_facts(orphans).await?,
            closed: closed_edges.into_iter().map(fact_of).collect(),
        })
    }

    /// Forget the words of the turns the worker read long ago.
    ///
    /// A turn is worth keeping while it waits to be read and while a
    /// reader may want to see what it taught. After that the raw text is
    /// the bulk of the memory and the risk of it, so the memory keeps the
    /// row, its identifier, and the message it belongs to, and forgets the
    /// words. The facts it taught stand on their own.
    pub async fn prune_episodes(&self, before: DateTime<Utc>) -> Result<u64, DbErr> {
        let cleared = memory_episode::Entity::update_many()
            .col_expr(memory_episode::Column::Text, Expr::value(String::new()))
            .col_expr(memory_episode::Column::Reply, Expr::value(String::new()))
            .col_expr(
                memory_episode::Column::Error,
                Expr::value(Value::String(None)),
            )
            .filter(memory_episode::Column::Processed.eq(true))
            .filter(memory_episode::Column::CreatedAt.lt(before.fixed_offset()))
            .filter(memory_episode::Column::Text.ne(""))
            .exec(&self.db)
            .await?;
        Ok(cleared.rows_affected)
    }

    /// Read the most recently changed nodes with their facts.
    async fn recent_nodes(&self, limit: u64) -> Result<Vec<MemoryNodeDto>, DbErr> {
        let nodes = self.list_nodes(limit).await?;
        self.with_facts(nodes).await
    }

    /// Attach the facts of every node.
    async fn with_facts(
        &self,
        nodes: Vec<memory_node::Model>,
    ) -> Result<Vec<MemoryNodeDto>, DbErr> {
        let ids = nodes.iter().map(|node| node.id).collect::<Vec<_>>();
        let mut facts = self.facts_of_many(&ids).await?;
        Ok(nodes
            .into_iter()
            .map(|node| {
                let own = facts.remove(&node.id).unwrap_or_default();
                node_of(node, own)
            })
            .collect())
    }

    /// Read the edges of one node that are true now.
    ///
    /// The relation is not part of the read. The write reads the current
    /// edges of the node and picks the relation through the one spelling
    /// the memory holds, so a relation learned under another spelling is
    /// closed with the rest rather than standing beside it.
    pub(crate) async fn current_edges(
        &self,
        subject: Uuid,
    ) -> Result<Vec<memory_edge::Model>, DbErr> {
        memory_edge::Entity::find()
            .filter(memory_edge::Column::SubjectId.eq(subject))
            .filter(memory_edge::Column::InvalidAt.is_null())
            .all(&self.db)
            .await
    }
}

/// Read the words of one text that are worth looking for.
///
/// A text is a sentence, so the words that carry it are the ones that are
/// not filler, are long enough to mean something, and still fit in one
/// search. The words keep their order and lose their doubles.
fn search_terms(text: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in text
        .to_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| word.chars().count() >= MIN_TERM_CHARS)
        .filter(|word| !STOP_WORDS.contains(word))
    {
        if !terms.iter().any(|term| term == word) {
            terms.push(word.to_string());
        }
        if terms.len() == MAX_SEARCH_TERMS {
            break;
        }
    }
    terms
}

/// How many words of one text name one concept.
fn named_terms(terms: &[String], node: &memory_node::Model) -> usize {
    let text = format!("{} {} {}", node.key, node.title, node.body).to_lowercase();
    terms
        .iter()
        .filter(|term| text.contains(term.as_str()))
        .count()
}

/// Match one column against one lowercased pattern without regard to case.
///
/// `LIKE` is case-sensitive on Postgres and case-insensitive on SQLite, so
/// the two backends would read the same search differently. Lowering both
/// sides settles it: `LOWER(column) LIKE '%pattern%'` reads the same way on
/// each of them.
fn lower_like<C>(column: C, pattern: &str) -> SimpleExpr
where
    C: ColumnTrait,
{
    let like = format!("%{pattern}%");
    Func::lower(Expr::col((column.entity_name(), column))).like(like)
}

/// The rank of one concept in the seed of a turn.
///
/// A concept the turn names is worth showing whether or not the memory
/// changed it recently, so the naming is the strongest signal. Freshness
/// and certainty decide the rest: a concept the memory changed this week
/// outranks one it changed last spring, and a fact the user stated twice
/// outranks a detail the assistant mentioned once.
fn seed_score(
    named: usize,
    node: &memory_node::Model,
    facts: &[MemoryFactDto],
    now: DateTime<Utc>,
) -> f64 {
    let named = named.min(FULLY_NAMED_TERMS) as f64 / FULLY_NAMED_TERMS as f64;
    let naming = NAMED_SCORE * named;
    let certainty = facts
        .iter()
        .filter(|fact| fact.current)
        .map(|fact| {
            let sightings = 1.0 + CONFIRMATION_WEIGHT * f64::from(fact.confirmations);
            f64::from(fact.importance) * f64::from(fact.confidence) * sightings
        })
        .fold(0.0, f64::max)
        .min(1.0);
    naming + freshness(node.updated_at.with_timezone(&Utc), now) + certainty * CERTAINTY_SCORE
}

/// How much the memory changed one concept just now, from zero to one.
fn freshness(changed_at: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let days = (now - changed_at).num_seconds().max(0) as f64 / 86_400.0;
    FRESHNESS_SCORE * 0.5f64.powf(days / FRESHNESS_HALF_LIFE_DAYS)
}

/// Render one concept of a seed.
///
/// The memory says how old a fact is when nothing confirmed it for a
/// while. An old fact is not a wrong one, but a model that reads the age
/// can question it, which is cheaper than trusting it.
fn seed_line(node: &memory_node::Model, facts: &[MemoryFactDto], now: DateTime<Utc>) -> String {
    let current = facts
        .iter()
        .filter(|fact| fact.current)
        .take(MAX_SEED_FACTS)
        .collect::<Vec<_>>();
    if current.is_empty() {
        let first = node
            .body
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim();
        return if first.is_empty() {
            format!("- {}", node.title)
        } else {
            format!("- {first}")
        };
    }
    let text = current
        .iter()
        .map(|fact| {
            let days = (now - fact.last_confirmed_at).num_days();
            if days >= STALE_AFTER_DAYS {
                format!(
                    "{} is {} (last confirmed {} days ago)",
                    fact.relation, fact.value, days
                )
            } else {
                format!("{} is {}", fact.relation, fact.value)
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("- {}: {text}", node.title)
}

/// Read the nodes that name the same thing twice.
///
/// The write path settles the name of a subject before it opens a concept,
/// so two keys for one thing are the marks of a memory that an older
/// daemon wrote, or of two concepts a reader opened before it learned one
/// of the names. The linter reports both of them, because the merge of the
/// editor is what settles it.
fn duplicate_nodes(nodes: &[memory_node::Model]) -> Vec<memory_node::Model> {
    let names = |node: &memory_node::Model| -> Vec<String> {
        vec![same_name(&node.key), same_name(&node.title)]
            .into_iter()
            .filter(|name| !name.is_empty())
            .collect()
    };
    let mut duplicates: Vec<memory_node::Model> = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let mine = names(node);
        let twin = nodes.iter().enumerate().any(|(other_index, other)| {
            other_index != index && names(other).iter().any(|name| mine.contains(name))
        });
        if twin {
            duplicates.push(node.clone());
        }
    }
    duplicates
}

/// Sort the facts of one node, current first and newest first within each.
fn sort_facts(facts: &mut [MemoryFactDto]) {
    facts.sort_by(|left, right| {
        right
            .current
            .cmp(&left.current)
            .then(right.valid_at.cmp(&left.valid_at))
    });
}

/// Convert one stored node and its facts into a DTO.
fn node_of(node: memory_node::Model, facts: Vec<MemoryFactDto>) -> MemoryNodeDto {
    MemoryNodeDto {
        id: node.id,
        key: node.key,
        title: node.title,
        body: node.body,
        facts,
        created_at: node.created_at.with_timezone(&Utc),
        updated_at: node.updated_at.with_timezone(&Utc),
    }
}

/// Convert one stored edge into a DTO.
fn fact_of(edge: memory_edge::Model) -> MemoryFactDto {
    MemoryFactDto {
        id: edge.id,
        relation: edge.relation,
        value: edge.value,
        current: edge.invalid_at.is_none(),
        confidence: edge.confidence,
        importance: edge.importance,
        confirmations: edge.confirmations,
        last_confirmed_at: edge.last_confirmed_at.with_timezone(&Utc),
        valid_at: edge.valid_at.with_timezone(&Utc),
        invalid_at: edge.invalid_at.map(|at| at.with_timezone(&Utc)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db;
    use crate::librarian::facts::{FactWrite, DEFAULT_CONFIDENCE, DEFAULT_IMPORTANCE};

    /// Build a store on an in memory database with the librarian schema.
    async fn store() -> LibrarianStore {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("an in memory database answers");
        db::init_schema(&db).await.expect("the schema is created");
        LibrarianStore::new(db)
    }

    /// Write one fact with the rating the reader answers by default.
    async fn write(
        store: &LibrarianStore,
        subject: Uuid,
        relation: &str,
        value: &str,
        at: DateTime<Utc>,
    ) {
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
                at,
            )
            .await
            .expect("the fact is written");
    }

    #[tokio::test]
    async fn a_lowercase_search_finds_a_concept_of_any_case() {
        let store = store().await;
        let node = store
            .upsert_node("user", "Alice Cooper", "Lives in Zurich", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "prefers", "Neovim", Utc::now()).await;

        // The pattern is lowered before the read, so a title, a body, a
        // relation, and a value all match whatever case they carry.
        assert_eq!(store.query("alice", 10).await.expect("query").len(), 1);
        assert_eq!(store.query("ZURICH", 10).await.expect("query").len(), 1);
        assert_eq!(store.query("neovim", 10).await.expect("query").len(), 1);
    }

    #[tokio::test]
    async fn a_search_of_the_words_of_a_relation_finds_it() {
        let store = store().await;
        let node = store
            .upsert_node("flurin", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "lives in", "Singapore", Utc::now()).await;

        assert_eq!(store.query("lives in", 10).await.expect("query").len(), 1);
        assert_eq!(store.query("lives_in", 10).await.expect("query").len(), 1);
    }

    #[tokio::test]
    async fn a_new_value_closes_the_older_fact_without_deleting_it() {
        let store = store().await;
        let node = store
            .upsert_node("user", "User", "", Utc::now())
            .await
            .expect("the node is stored");
        write(&store, node, "lives_in", "Berlin", Utc::now()).await;
        write(&store, node, "lives_in", "Zurich", Utc::now()).await;

        let facts = store.facts_of(node).await.expect("the facts are read");
        assert_eq!(facts.len(), 2);
        assert_eq!(facts.iter().filter(|fact| fact.current).count(), 1);
        assert_eq!(
            facts
                .iter()
                .find(|fact| fact.current)
                .map(|fact| fact.value.as_str()),
            Some("Zurich")
        );
        assert!(facts
            .iter()
            .any(|fact| fact.value == "Berlin" && !fact.current && fact.invalid_at.is_some()));
    }

    #[tokio::test]
    async fn the_facts_of_a_message_are_read_back() {
        let store = store().await;
        let message = Uuid::new_v4();
        let episode = Uuid::new_v4();
        store
            .enqueue_episode(
                episode,
                Some(message),
                "I live in Zurich",
                "Noted.",
                None,
                Utc::now(),
            )
            .await
            .expect("the episode is stored");
        let node = store
            .upsert_node("flurin", "Flurin", "", Utc::now())
            .await
            .expect("the node is stored");
        store
            .add_fact(
                node,
                FactWrite {
                    relation: "lives_in",
                    value: "Zurich",
                    confidence: DEFAULT_CONFIDENCE,
                    importance: DEFAULT_IMPORTANCE,
                    episode: Some(episode),
                },
                Utc::now(),
            )
            .await
            .expect("the fact is stored");

        let learned = store
            .memory_of_messages(&[message])
            .await
            .expect("the memory is read");
        let facts = learned.get(&message).expect("the message taught a fact");
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].concept, "Flurin");
        assert_eq!(facts[0].relation, "lives_in");
        assert_eq!(facts[0].value, "Zurich");
    }

    #[tokio::test]
    async fn a_message_the_memory_never_read_teaches_nothing() {
        let store = store().await;
        let learned = store
            .memory_of_messages(&[Uuid::new_v4()])
            .await
            .expect("the memory is read");

        assert!(learned.is_empty());
    }

    #[tokio::test]
    async fn the_seed_shows_the_concept_the_turn_is_about() {
        let store = store().await;
        let now = Utc::now();
        let user = store
            .upsert_node("user", "Flurin", "", now)
            .await
            .expect("the node is stored");
        write(&store, user, "lives_in", "Singapore", now).await;
        // The editor is the newest concept of the store, so a seed that
        // only read the newest would show it and hide the user.
        let editor = store
            .upsert_node("editor", "Editor", "", now)
            .await
            .expect("the node is stored");
        write(&store, editor, "name", "Neovim", now).await;

        let seed = store
            .seed("which editor do I use?", 1)
            .await
            .expect("the seed is read");

        assert!(seed.contains("Editor"), "the turn named the editor");
        assert!(!seed.contains("Flurin"));
    }

    #[tokio::test]
    async fn the_seed_falls_back_on_what_the_memory_changed_last() {
        let store = store().await;
        let now = Utc::now();
        let user = store
            .upsert_node("user", "Flurin", "", now)
            .await
            .expect("the node is stored");
        write(&store, user, "lives_in", "Singapore", now).await;

        let seed = store
            .seed("hello there", 4)
            .await
            .expect("the seed is read");

        assert!(seed.starts_with(SEED_HEADER));
        assert!(seed.contains("Flurin: lives_in is Singapore"));
    }

    #[tokio::test]
    async fn the_seed_shows_a_fact_the_turn_names() {
        let store = store().await;
        let now = Utc::now();
        let project = store
            .upsert_node("project_alice", "Project Alice", "", now)
            .await
            .expect("the node is stored");
        write(&store, project, "uses", "Tokio", now).await;

        let seed = store
            .seed("what does the alice project run on?", 1)
            .await
            .expect("the seed is read");

        assert!(seed.contains("uses is Tokio"));
    }

    #[tokio::test]
    async fn the_seed_says_when_a_fact_is_old() {
        let store = store().await;
        let old = Utc::now() - chrono::Duration::days(120);
        let user = store
            .upsert_node("user", "Flurin", "", old)
            .await
            .expect("the node is stored");
        write(&store, user, "lives_in", "Singapore", old).await;

        let seed = store.seed("user", 1).await.expect("the seed is read");

        // A fact nothing confirmed for months reads as old, so the model
        // can question it rather than trust it.
        assert!(seed.contains("last confirmed 120 days ago"), "got {seed}");
    }

    #[tokio::test]
    async fn the_seed_leaves_a_quiet_fact_out() {
        let store = store().await;
        let now = Utc::now();
        let user = store
            .upsert_node("user", "Flurin", "", now)
            .await
            .expect("the node is stored");
        write(&store, user, "lives_in", "Singapore", now).await;
        let machine = store
            .upsert_node("machine", "Machine", "", now)
            .await
            .expect("the node is stored");
        write(&store, machine, "runs", "NixOS", now).await;

        let seed = store.seed("user", 1).await.expect("the seed is read");

        let mut lines = seed.lines().skip(1);
        assert!(lines.next().is_some(), "one concept is shown");
        assert!(lines.next().is_none(), "only one concept is shown");
    }

    #[tokio::test]
    async fn the_counts_tell_now_from_the_history() {
        let store = store().await;
        let now = Utc::now();
        let user = store
            .upsert_node("user", "Flurin", "", now)
            .await
            .expect("the node is stored");
        write(&store, user, "lives_in", "Berlin", now).await;
        write(&store, user, "lives_in", "Zurich", now).await;
        write(&store, user, "lives_in", "zurich", now).await;

        assert_eq!(store.count_edges().await.expect("the count reads"), 2);
        assert_eq!(
            store.count_current_edges().await.expect("the count reads"),
            1
        );
        assert_eq!(
            store
                .count_confirmed_edges()
                .await
                .expect("the count reads"),
            1
        );
    }

    #[tokio::test]
    async fn pruning_keeps_the_fact_and_forgets_the_words() {
        let store = store().await;
        let old = Utc::now() - chrono::Duration::days(90);
        let episode = Uuid::new_v4();
        store
            .enqueue_episode(
                episode,
                Some(Uuid::new_v4()),
                "I live in Zurich",
                "Noted.",
                None,
                old,
            )
            .await
            .expect("the episode is stored");
        store
            .finish_episode(episode, None)
            .await
            .expect("the episode is read");

        let cleared = store
            .prune_episodes(Utc::now() - chrono::Duration::days(30))
            .await
            .expect("the memory is pruned");

        assert_eq!(cleared, 1);
        let stored = store.next_episode().await.expect("the queue is read");
        assert!(stored.is_none(), "a read episode never waits again");
    }

    #[tokio::test]
    async fn pruning_leaves_a_turn_the_worker_has_not_read() {
        let store = store().await;
        let old = Utc::now() - chrono::Duration::days(90);
        let episode = Uuid::new_v4();
        store
            .enqueue_episode(episode, None, "I live in Zurich", "Noted.", None, old)
            .await
            .expect("the episode is stored");

        let cleared = store
            .prune_episodes(Utc::now() - chrono::Duration::days(30))
            .await
            .expect("the memory is pruned");

        assert_eq!(cleared, 0);
        let waiting = store
            .next_episode()
            .await
            .expect("the queue is read")
            .expect("the turn still waits");
        assert_eq!(waiting.text, "I live in Zurich");
    }

    #[tokio::test]
    async fn the_lint_reports_two_keys_for_one_thing() {
        let store = store().await;
        let now = Utc::now();
        store
            .upsert_node("user", "Flurin", "", now)
            .await
            .expect("the node is stored");
        store
            .upsert_node("flurin", "Flurin", "", now)
            .await
            .expect("the second node is stored");

        let report = store.lint(50).await.expect("the report is read");

        assert_eq!(report.duplicates.len(), 2);
        assert_eq!(report.orphans.len(), 2);
    }
}
