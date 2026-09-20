//! Intent service of the daemon.
//! This module owns the intents, their entities, and the values of the
//! closed ones. A write replaces the whole entity set of the intent.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};
use uuid::Uuid;

use alice_core::dto::{EntityKindDto, IntentDto, IntentEntityDto};

use crate::intent::entity::{entity_value, intent, intent_entity, intent_example};
use crate::intent::error::SaveIntentError;
use crate::intent::input::{EntityInput, IntentInput};

/// The catalog the daemon read, and the revision it read at.
#[derive(Clone, Debug)]
struct CachedCatalog {
    /// The revision of the configuration the catalog belongs to.
    revision: u64,
    /// The intents the daemon read.
    intents: Vec<IntentDto>,
}

/// Service that stores the intent configuration.
///
/// The catalog a resolver reads lives in memory: every turn needs it, and
/// only a write replaces it. The service bumps a revision on every write
/// and reads the table again on the next turn that follows one, so a
/// saved intent applies to the next message without a read per message.
#[derive(Clone, Debug)]
pub struct IntentService {
    db: DatabaseConnection,
    /// The catalog the daemon read last, or none before the first read.
    cache: Arc<Mutex<Option<CachedCatalog>>>,
    /// Counter that changes on every write.
    revision: Arc<AtomicU64>,
}

impl IntentService {
    /// Create a new intent service.
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            cache: Arc::new(Mutex::new(None)),
            revision: Arc::new(AtomicU64::new(0)),
        }
    }

    /// List the intents in name order, with their entities.
    ///
    /// The catalog comes from memory while no write has happened since
    /// the last read.
    pub async fn list_intents(&self) -> Result<Vec<IntentDto>, DbErr> {
        let revision = self.revision.load(Ordering::SeqCst);
        if let Some(cached) = self.cached(revision) {
            return Ok(cached);
        }
        let intents = self.load_intents(None).await?;
        *self.lock() = Some(CachedCatalog {
            revision,
            intents: intents.clone(),
        });
        Ok(intents)
    }

    /// The catalog in memory, when it belongs to the given revision.
    fn cached(&self, revision: u64) -> Option<Vec<IntentDto>> {
        let cache = self.lock();
        cache
            .as_ref()
            .filter(|cached| cached.revision == revision)
            .map(|cached| cached.intents.clone())
    }

    /// Lock the catalog in memory.
    fn lock(&self) -> MutexGuard<'_, Option<CachedCatalog>> {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Report a write, so the next read takes the table again.
    fn touch(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// Find one intent with its entities, or null when it does not exist.
    pub async fn find_intent(&self, id: Uuid) -> Result<Option<IntentDto>, DbErr> {
        let mut found = self.load_intents(Some(id)).await?;
        Ok(found.pop())
    }

    /// Create one intent and return it with its stored entities.
    pub async fn create_intent(
        &self,
        input: IntentInput,
        at: DateTime<Utc>,
    ) -> Result<IntentDto, SaveIntentError> {
        let input = input.normalize()?;
        self.check_name_free(&input.name, None).await?;

        let id = Uuid::new_v4();
        let txn = self.db.begin().await?;
        intent::Entity::insert(intent::ActiveModel {
            id: Set(id),
            name: Set(input.name.clone()),
            description: Set(input.description.clone()),
            command: Set(input.command.clone()),
            created_at: Set(at.into()),
            updated_at: Set(at.into()),
        })
        .exec(&txn)
        .await?;
        insert_entities(&txn, id, &input.entities, at).await?;
        insert_examples(&txn, id, &input.examples).await?;
        txn.commit().await?;
        self.touch();

        self.find_intent(id)
            .await?
            .ok_or(SaveIntentError::NotFound { id })
    }

    /// Replace one intent with the sent values and return it.
    pub async fn update_intent(
        &self,
        id: Uuid,
        input: IntentInput,
        at: DateTime<Utc>,
    ) -> Result<IntentDto, SaveIntentError> {
        let input = input.normalize()?;
        if self.find_intent(id).await?.is_none() {
            return Err(SaveIntentError::NotFound { id });
        }
        self.check_name_free(&input.name, Some(id)).await?;

        let txn = self.db.begin().await?;
        intent::Entity::update(intent::ActiveModel {
            id: Set(id),
            name: Set(input.name.clone()),
            description: Set(input.description.clone()),
            command: Set(input.command.clone()),
            updated_at: Set(at.into()),
            ..Default::default()
        })
        .exec(&txn)
        .await?;
        delete_entities(&txn, id).await?;
        insert_entities(&txn, id, &input.entities, at).await?;
        delete_examples(&txn, id).await?;
        insert_examples(&txn, id, &input.examples).await?;
        txn.commit().await?;
        self.touch();

        self.find_intent(id)
            .await?
            .ok_or(SaveIntentError::NotFound { id })
    }

    /// Read the names of the given intents, by identifier text.
    ///
    /// A stored message keeps the identifier of its intent. The reader
    /// turns those identifiers into the names the interface shows.
    pub async fn names_for(&self, ids: &[String]) -> Result<HashMap<String, String>, DbErr> {
        let parsed: Vec<Uuid> = ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();
        if parsed.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = intent::Entity::find()
            .filter(intent::Column::Id.is_in(parsed))
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| (row.id.to_string(), row.name))
            .collect())
    }

    /// Delete one intent with its entities. It returns false when it is gone.
    pub async fn delete_intent(&self, id: Uuid) -> Result<bool, SaveIntentError> {
        let txn = self.db.begin().await?;
        let removed = intent::Entity::delete_by_id(id).exec(&txn).await?;
        delete_entities(&txn, id).await?;
        delete_examples(&txn, id).await?;
        txn.commit().await?;
        if removed.rows_affected > 0 {
            self.touch();
        }
        Ok(removed.rows_affected > 0)
    }

    /// Fail when another intent already uses the name.
    async fn check_name_free(&self, name: &str, keep: Option<Uuid>) -> Result<(), SaveIntentError> {
        let used = intent::Entity::find()
            .filter(intent::Column::Name.eq(name))
            .one(&self.db)
            .await?;
        match used {
            Some(row) if Some(row.id) != keep => Err(SaveIntentError::NameTaken {
                name: name.to_string(),
            }),
            _ => Ok(()),
        }
    }

    /// Load the intents and attach their entities and entity values.
    async fn load_intents(&self, only: Option<Uuid>) -> Result<Vec<IntentDto>, DbErr> {
        let mut query = intent::Entity::find();
        if let Some(id) = only {
            query = query.filter(intent::Column::Id.eq(id));
        }
        let rows = query
            .order_by_asc(intent::Column::Name)
            .all(&self.db)
            .await?;

        let entities = self.load_entities(&rows).await?;
        let examples = self.load_examples(&rows).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let entities = entities.get(&row.id).cloned().unwrap_or_default();
                let examples = examples.get(&row.id).cloned().unwrap_or_default();
                IntentDto {
                    id: row.id,
                    name: row.name,
                    description: row.description,
                    command: row.command,
                    entities,
                    examples,
                    created_at: row.created_at.into(),
                    updated_at: row.updated_at.into(),
                }
            })
            .collect())
    }

    /// Load the entities and their values for the given intents.
    async fn load_entities(
        &self,
        rows: &[intent::Model],
    ) -> Result<HashMap<Uuid, Vec<IntentEntityDto>>, DbErr> {
        let intent_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        if intent_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let entity_rows = intent_entity::Entity::find()
            .filter(intent_entity::Column::IntentId.is_in(intent_ids))
            .order_by_asc(intent_entity::Column::Name)
            .all(&self.db)
            .await?;

        let mut values = self.load_values(&entity_rows).await?;
        let mut grouped: HashMap<Uuid, Vec<IntentEntityDto>> = HashMap::new();
        for row in entity_rows {
            let values = values.remove(&row.id).unwrap_or_default();
            grouped
                .entry(row.intent_id)
                .or_default()
                .push(IntentEntityDto {
                    id: row.id,
                    name: row.name,
                    kind: EntityKindDto::from_stored(&row.kind),
                    values,
                    script: row.script,
                    required: row.required,
                });
        }
        Ok(grouped)
    }

    /// Load the phrases of the given intents.
    async fn load_examples(
        &self,
        rows: &[intent::Model],
    ) -> Result<HashMap<Uuid, Vec<String>>, DbErr> {
        let intent_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
        if intent_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = intent_example::Entity::find()
            .filter(intent_example::Column::IntentId.is_in(intent_ids))
            .order_by_asc(intent_example::Column::Text)
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<Uuid, Vec<String>> = HashMap::new();
        for row in rows {
            grouped.entry(row.intent_id).or_default().push(row.text);
        }
        Ok(grouped)
    }

    /// Load the values of the given closed entities.
    async fn load_values(
        &self,
        entities: &[intent_entity::Model],
    ) -> Result<HashMap<Uuid, Vec<String>>, DbErr> {
        let entity_ids: Vec<Uuid> = entities.iter().map(|row| row.id).collect();
        if entity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = entity_value::Entity::find()
            .filter(entity_value::Column::EntityId.is_in(entity_ids))
            .order_by_asc(entity_value::Column::Value)
            .all(&self.db)
            .await?;

        let mut grouped: HashMap<Uuid, Vec<String>> = HashMap::new();
        for row in rows {
            grouped.entry(row.entity_id).or_default().push(row.value);
        }
        Ok(grouped)
    }
}

/// Insert the entities of one intent and their closed values.
async fn insert_entities<C: ConnectionTrait>(
    db: &C,
    intent_id: Uuid,
    entities: &[EntityInput],
    at: DateTime<Utc>,
) -> Result<(), DbErr> {
    for entity in entities {
        let entity_id = Uuid::new_v4();
        intent_entity::Entity::insert(intent_entity::ActiveModel {
            id: Set(entity_id),
            intent_id: Set(intent_id),
            name: Set(entity.name.clone()),
            kind: Set(entity.kind.as_str().to_string()),
            script: Set(entity.script.clone()),
            required: Set(entity.required),
            created_at: Set(at.into()),
        })
        .exec(db)
        .await?;

        for value in &entity.values {
            entity_value::Entity::insert(entity_value::ActiveModel {
                id: Set(Uuid::new_v4()),
                entity_id: Set(entity_id),
                value: Set(value.clone()),
            })
            .exec(db)
            .await?;
        }
    }
    Ok(())
}

/// Insert the phrases of one intent.
async fn insert_examples<C: ConnectionTrait>(
    db: &C,
    intent_id: Uuid,
    examples: &[String],
) -> Result<(), DbErr> {
    for example in examples {
        intent_example::Entity::insert(intent_example::ActiveModel {
            id: Set(Uuid::new_v4()),
            intent_id: Set(intent_id),
            text: Set(example.clone()),
        })
        .exec(db)
        .await?;
    }
    Ok(())
}

/// Delete the phrases of one intent.
async fn delete_examples<C: ConnectionTrait>(db: &C, intent_id: Uuid) -> Result<(), DbErr> {
    intent_example::Entity::delete_many()
        .filter(intent_example::Column::IntentId.eq(intent_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Delete the entities of one intent and their closed values.
async fn delete_entities<C: ConnectionTrait>(db: &C, intent_id: Uuid) -> Result<(), DbErr> {
    let entity_ids: Vec<Uuid> = intent_entity::Entity::find()
        .filter(intent_entity::Column::IntentId.eq(intent_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.id)
        .collect();

    if !entity_ids.is_empty() {
        entity_value::Entity::delete_many()
            .filter(entity_value::Column::EntityId.is_in(entity_ids))
            .exec(db)
            .await?;
    }

    intent_entity::Entity::delete_many()
        .filter(intent_entity::Column::IntentId.eq(intent_id))
        .exec(db)
        .await?;
    Ok(())
}
