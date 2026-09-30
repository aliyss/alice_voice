//! Schema upgrades for a database that an older daemon created.
//! This module keeps the data of an older database and adds what is missing.

use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, Statement,
};

use crate::conversation::entity::message;
use crate::conversation::ConversationService;

/// The table that stores the handled messages.
const MESSAGE_TABLE: &str = "chat_message";

/// The column that links a message to its conversation.
const CONVERSATION_COLUMN: &str = "conversation_id";

/// The column that stores how the daemon read one turn.
const META_COLUMN: &str = "meta";

/// The table that stores the entities of an intent.
const ENTITY_TABLE: &str = "intent_entity";

/// The column that stores the script of a script entity.
const ENTITY_SCRIPT_COLUMN: &str = "script";

/// The column that stores whether an intent needs a value of an entity.
const ENTITY_REQUIRED_COLUMN: &str = "required";

/// The table that stores the episodes the memory reads.
const EPISODE_TABLE: &str = "memory_episode";

/// The column that links an episode to the message of its turn.
const EPISODE_MESSAGE_COLUMN: &str = "message_id";

/// The column that counts the attempts of the worker on one episode.
const EPISODE_ATTEMPTS_COLUMN: &str = "attempts";

/// The column that stores when the worker may try an episode again.
const EPISODE_RETRY_COLUMN: &str = "next_attempt_at";

/// The table that stores one relation of one memory node.
const EDGE_TABLE: &str = "memory_edge";

/// The column that stores how sure the reader was of a fact.
const EDGE_CONFIDENCE_COLUMN: &str = "confidence";

/// The column that stores how much a fact is worth.
const EDGE_IMPORTANCE_COLUMN: &str = "importance";

/// The column that counts the turns that taught a fact again.
const EDGE_CONFIRMATIONS_COLUMN: &str = "confirmations";

/// The column that stores when the memory last saw a fact.
const EDGE_CONFIRMED_AT_COLUMN: &str = "last_confirmed_at";

/// The table that stores the known names of one memory node.
const ALIAS_TABLE: &str = "memory_alias";

/// Apply every upgrade that the given database needs.
pub async fn apply(db: &DatabaseConnection) -> Result<(), DbErr> {
    migrate_message_conversation(db).await?;
    migrate_message_meta(db).await?;
    migrate_entity_script(db).await?;
    migrate_entity_required(db).await?;
    migrate_episode_message(db).await?;
    migrate_episode_retry(db).await?;
    migrate_fact_certainty(db).await?;
    create_alias_table(db).await?;
    create_message_index(db).await
}

/// Add the columns that rate one memory fact.
///
/// A daemon that is older than the rating wrote facts without a
/// confidence, an importance, a count of the turns that confirmed them,
/// and the time the memory last saw them. An older fact reads as one the
/// memory saw once and confirmed when it became true, so the seed of a
/// turn reports the age the fact really has rather than the time of the
/// upgrade.
async fn migrate_fact_certainty(db: &DatabaseConnection) -> Result<(), DbErr> {
    if !has_column(db, EDGE_TABLE, EDGE_CONFIDENCE_COLUMN).await? {
        add_column(
            db,
            EDGE_TABLE,
            EDGE_CONFIDENCE_COLUMN,
            "REAL NOT NULL DEFAULT 0.6",
        )
        .await?;
    }
    if !has_column(db, EDGE_TABLE, EDGE_IMPORTANCE_COLUMN).await? {
        add_column(
            db,
            EDGE_TABLE,
            EDGE_IMPORTANCE_COLUMN,
            "REAL NOT NULL DEFAULT 0.5",
        )
        .await?;
    }
    if !has_column(db, EDGE_TABLE, EDGE_CONFIRMATIONS_COLUMN).await? {
        add_column(
            db,
            EDGE_TABLE,
            EDGE_CONFIRMATIONS_COLUMN,
            "INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
    }
    if !has_column(db, EDGE_TABLE, EDGE_CONFIRMED_AT_COLUMN).await? {
        // The column arrives empty, because a constant default would date
        // every old fact to the upgrade. The backfill gives each of them
        // the time it became true instead, and the entity always writes
        // the column on a new fact.
        add_column(db, EDGE_TABLE, EDGE_CONFIRMED_AT_COLUMN, timestamp_type(db)).await?;
        let backfill = format!(
            "UPDATE {EDGE_TABLE} SET {EDGE_CONFIRMED_AT_COLUMN} = valid_at \
             WHERE {EDGE_CONFIRMED_AT_COLUMN} IS NULL"
        );
        db.execute(Statement::from_string(db.get_database_backend(), backfill))
            .await?;
    }
    Ok(())
}

/// Add the columns that let the worker read an episode again.
///
/// A daemon that is older than the retry gave up on an episode the first
/// time the model server did not answer, so the turn was lost. An older
/// episode reads as one the worker never tried, and the worker reads it
/// again on the next pass.
async fn migrate_episode_retry(db: &DatabaseConnection) -> Result<(), DbErr> {
    if !has_column(db, EPISODE_TABLE, EPISODE_ATTEMPTS_COLUMN).await? {
        add_column(
            db,
            EPISODE_TABLE,
            EPISODE_ATTEMPTS_COLUMN,
            "INTEGER NOT NULL DEFAULT 0",
        )
        .await?;
    }
    if !has_column(db, EPISODE_TABLE, EPISODE_RETRY_COLUMN).await? {
        add_column(db, EPISODE_TABLE, EPISODE_RETRY_COLUMN, timestamp_type(db)).await?;
    }
    Ok(())
}

/// Create the table that stores the known names of one memory node.
///
/// The table is new, so this runs on every database and changes nothing
/// on one that already holds it. A memory without a row learns the names
/// of the concepts it already holds as it reads the turns that use them.
async fn create_alias_table(db: &DatabaseConnection) -> Result<(), DbErr> {
    let id_type = match db.get_database_backend() {
        DatabaseBackend::Postgres => "UUID",
        _ => "TEXT",
    };
    for statement in [
        format!(
            "CREATE TABLE IF NOT EXISTS {ALIAS_TABLE} \
             (alias TEXT PRIMARY KEY NOT NULL, node_id {id_type} NOT NULL)"
        ),
        format!("CREATE INDEX IF NOT EXISTS idx_memory_alias_node ON {ALIAS_TABLE}(node_id)"),
    ] {
        db.execute(Statement::from_string(db.get_database_backend(), statement))
            .await?;
    }
    Ok(())
}

/// Add one column to one table.
async fn add_column(
    db: &DatabaseConnection,
    table: &str,
    column: &str,
    column_type: &str,
) -> Result<(), DbErr> {
    let alter = format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}");
    db.execute(Statement::from_string(db.get_database_backend(), alter))
        .await?;
    Ok(())
}

/// The type one timestamp column takes on this backend.
fn timestamp_type(db: &DatabaseConnection) -> &'static str {
    match db.get_database_backend() {
        DatabaseBackend::Postgres => "TIMESTAMPTZ",
        _ => "TEXT",
    }
}

/// Add the column that links an episode to the message of its turn.
///
/// A daemon that is older than the memory mark stored episodes without the
/// message they came from, so the old rows report no turn and the
/// transcript shows no mark for them. The column is nullable and both
/// backends accept it.
async fn migrate_episode_message(db: &DatabaseConnection) -> Result<(), DbErr> {
    if has_column(db, EPISODE_TABLE, EPISODE_MESSAGE_COLUMN).await? {
        return Ok(());
    }
    let column_type = match db.get_database_backend() {
        DatabaseBackend::Postgres => "UUID",
        _ => "TEXT",
    };
    let alter =
        format!("ALTER TABLE {EPISODE_TABLE} ADD COLUMN {EPISODE_MESSAGE_COLUMN} {column_type}");
    db.execute(Statement::from_string(db.get_database_backend(), alter))
        .await?;
    Ok(())
}

/// Add the column that stores whether an entity is required.
///
/// A daemon that is older than the required flag read every entity of an
/// intent, so the flag defaults to yes and the old entities keep their
/// behaviour. Both backends take a column with a default, so the old rows
/// answer without a write.
async fn migrate_entity_required(db: &DatabaseConnection) -> Result<(), DbErr> {
    if has_column(db, ENTITY_TABLE, ENTITY_REQUIRED_COLUMN).await? {
        return Ok(());
    }
    let default = match db.get_database_backend() {
        DatabaseBackend::Postgres => "TRUE",
        _ => "1",
    };
    let alter = format!(
        "ALTER TABLE {ENTITY_TABLE} ADD COLUMN {ENTITY_REQUIRED_COLUMN} BOOLEAN NOT NULL DEFAULT {default}"
    );
    db.execute(Statement::from_string(db.get_database_backend(), alter))
        .await?;
    Ok(())
}

/// Add the column that stores the script of a script entity.
///
/// A daemon that is older than the script kind stored no script. The
/// column is nullable and both backends accept it, so an entity of an
/// older database keeps its kind and reports no script.
async fn migrate_entity_script(db: &DatabaseConnection) -> Result<(), DbErr> {
    if has_column(db, ENTITY_TABLE, ENTITY_SCRIPT_COLUMN).await? {
        return Ok(());
    }
    let alter = format!("ALTER TABLE {ENTITY_TABLE} ADD COLUMN {ENTITY_SCRIPT_COLUMN} TEXT");
    db.execute(Statement::from_string(db.get_database_backend(), alter))
        .await?;
    Ok(())
}

/// Add the column that stores how the daemon read a turn.
///
/// A daemon that is older than the turn metadata stored no metadata. The
/// column is nullable and both backends accept it, so the old rows keep
/// their fields and report no metadata. This runs after the conversation
/// column, so the message table always exists by the time it runs.
async fn migrate_message_meta(db: &DatabaseConnection) -> Result<(), DbErr> {
    if has_column(db, MESSAGE_TABLE, META_COLUMN).await? {
        return Ok(());
    }
    let alter = format!("ALTER TABLE {MESSAGE_TABLE} ADD COLUMN {META_COLUMN} TEXT");
    db.execute(Statement::from_string(db.get_database_backend(), alter))
        .await?;
    Ok(())
}

/// Create the index of the message table on the conversation column.
///
/// The index needs the column, so this function runs after the column
/// exists. The schema init cannot create it, because an older database
/// has the message table without the column.
async fn create_message_index(db: &DatabaseConnection) -> Result<(), DbErr> {
    let sql = format!(
        "CREATE INDEX IF NOT EXISTS idx_chat_message_conversation \
         ON {MESSAGE_TABLE}({CONVERSATION_COLUMN}, created_at)"
    );
    db.execute(Statement::from_string(db.get_database_backend(), sql))
        .await?;
    Ok(())
}

/// Add `conversation_id` to the message table and link the old rows.
///
/// A daemon that is older than the conversation feature created the message
/// table without a conversation. This function adds the column and puts the
/// old messages into one imported conversation, so no message loses its home.
async fn migrate_message_conversation(db: &DatabaseConnection) -> Result<(), DbErr> {
    if has_column(db, MESSAGE_TABLE, CONVERSATION_COLUMN).await? {
        return Ok(());
    }

    // 1. Add the column. Both backends accept a nullable column.
    let column_type = match db.get_database_backend() {
        DatabaseBackend::Postgres => "UUID",
        _ => "TEXT",
    };
    let alter =
        format!("ALTER TABLE {MESSAGE_TABLE} ADD COLUMN {CONVERSATION_COLUMN} {column_type}");
    db.execute(Statement::from_string(db.get_database_backend(), alter))
        .await?;

    // 2. Put the old messages into one imported conversation. The title
    //    follows the rule of a conversation and comes from the oldest message.
    let Some(first_text) = oldest_message_text(db).await? else {
        return Ok(());
    };
    let conversations = ConversationService::new(db.clone());
    let conversation = conversations
        .create_conversation(&first_text, Utc::now())
        .await?;

    // 3. Link the old rows. The entity keeps the identifier encoding of the
    //    backend, so the write matches the reads of the message service.
    message::Entity::update_many()
        .col_expr(
            message::Column::ConversationId,
            Expr::value(conversation.id),
        )
        .filter(message::Column::ConversationId.is_null())
        .exec(db)
        .await?;
    Ok(())
}

/// Check whether a table has a column.
///
/// The probe runs one select. A missing column makes the select fail, and
/// that failure is the answer. The table exists, because the schema init
/// runs before this module.
async fn has_column(db: &DatabaseConnection, table: &str, column: &str) -> Result<bool, DbErr> {
    let probe = format!("SELECT {column} FROM {table} LIMIT 1");
    let result = db
        .execute(Statement::from_string(db.get_database_backend(), probe))
        .await;
    Ok(result.is_ok())
}

/// Read the text of the oldest stored message, or null when none exists.
async fn oldest_message_text(db: &DatabaseConnection) -> Result<Option<String>, DbErr> {
    let sql = format!("SELECT text FROM {MESSAGE_TABLE} ORDER BY created_at ASC LIMIT 1");
    let rows = db
        .query_all(Statement::from_string(db.get_database_backend(), sql))
        .await?;
    match rows.into_iter().next() {
        Some(row) => Ok(Some(row.try_get("", "text")?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    /// Create the tables of an older daemon, without the newer columns.
    async fn old_database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("an in memory database answers");
        for statement in [
            "CREATE TABLE chat_message (id TEXT PRIMARY KEY NOT NULL, text TEXT NOT NULL, \
             created_at TEXT NOT NULL)",
            "CREATE TABLE intent_entity (id TEXT PRIMARY KEY NOT NULL, intent_id TEXT NOT NULL, \
             name TEXT NOT NULL, kind TEXT NOT NULL, script TEXT, created_at TEXT NOT NULL)",
            "INSERT INTO intent_entity (id, intent_id, name, kind, script, created_at) \
             VALUES ('entity', 'intent', 'city', 'open', NULL, '2026-01-01T00:00:00Z')",
            "CREATE TABLE memory_episode (id TEXT PRIMARY KEY NOT NULL, text TEXT NOT NULL, \
             reply TEXT NOT NULL, intent TEXT, processed BOOLEAN NOT NULL DEFAULT 0, \
             error TEXT, created_at TEXT NOT NULL)",
            "INSERT INTO memory_episode (id, text, reply, intent, processed, error, \
             created_at) VALUES ('00000000-0000-0000-0000-000000000001', \
             'I live in Zurich', 'Noted.', NULL, 0, NULL, '2026-01-01T00:00:00Z')",
            "CREATE TABLE memory_edge (id TEXT PRIMARY KEY NOT NULL, \
             subject_id TEXT NOT NULL, relation TEXT NOT NULL, value TEXT NOT NULL, \
             valid_at TEXT NOT NULL, invalid_at TEXT, superseded_by TEXT, \
             source_episode TEXT)",
            "INSERT INTO memory_edge (id, subject_id, relation, value, valid_at) VALUES \
             ('00000000-0000-0000-0000-000000000002', \
             '00000000-0000-0000-0000-000000000003', 'lives_in', 'Zurich', \
             '2026-01-01T00:00:00Z')",
        ] {
            db.execute(Statement::from_string(
                db.get_database_backend(),
                statement.to_string(),
            ))
            .await
            .expect("the old schema is created");
        }
        db
    }

    /// Count the stored episodes and the ones that name a message.
    async fn episode_count(db: &DatabaseConnection) -> (i64, i64) {
        let sql = format!(
            "SELECT COUNT(*) AS all_episodes, COUNT({EPISODE_MESSAGE_COLUMN}) AS named \
             FROM {EPISODE_TABLE}"
        );
        let rows = db
            .query_all(Statement::from_string(db.get_database_backend(), sql))
            .await
            .expect("the episodes are read");
        let row = rows.into_iter().next().expect("the episodes are read");
        (
            row.try_get("", "all_episodes").expect("the count reads"),
            row.try_get("", "named").expect("the count reads"),
        )
    }

    /// Read the rating of the only stored fact: its confirmations, its
    /// confidence, and the time the memory last saw it.
    async fn stored_fact(db: &DatabaseConnection) -> (i64, f64, String) {
        let sql = format!(
            "SELECT {EDGE_CONFIRMATIONS_COLUMN} AS confirmations, \
             {EDGE_CONFIDENCE_COLUMN} AS confidence, \
             {EDGE_CONFIRMED_AT_COLUMN} AS confirmed_at FROM {EDGE_TABLE}"
        );
        let rows = db
            .query_all(Statement::from_string(db.get_database_backend(), sql))
            .await
            .expect("the fact is read");
        let row = rows.into_iter().next().expect("the fact is read");
        (
            row.try_get("", "confirmations").expect("the count reads"),
            row.try_get("", "confidence").expect("the rating reads"),
            row.try_get("", "confirmed_at").expect("the time reads"),
        )
    }

    /// Read the attempts of the only stored episode and whether it waits.
    async fn stored_attempts(db: &DatabaseConnection) -> (i64, i64) {
        let sql = format!(
            "SELECT {EPISODE_ATTEMPTS_COLUMN} AS attempts, \
             ({EPISODE_RETRY_COLUMN} IS NULL) AS waiting FROM {EPISODE_TABLE}"
        );
        let rows = db
            .query_all(Statement::from_string(db.get_database_backend(), sql))
            .await
            .expect("the episode is read");
        let row = rows.into_iter().next().expect("the episode is read");
        (
            row.try_get("", "attempts").expect("the count reads"),
            row.try_get("", "waiting").expect("the wait reads"),
        )
    }

    /// Read one column of the only stored entity.
    async fn stored_required(db: &DatabaseConnection) -> bool {
        let sql = format!("SELECT {ENTITY_REQUIRED_COLUMN} FROM {ENTITY_TABLE}");
        let rows = db
            .query_all(Statement::from_string(db.get_database_backend(), sql))
            .await
            .expect("the entity is there");
        let row = rows.into_iter().next().expect("the entity is there");
        row.try_get("", ENTITY_REQUIRED_COLUMN)
            .expect("the column reads as a flag")
    }

    #[tokio::test]
    async fn the_required_column_keeps_the_entities_of_an_older_database() {
        let db = old_database().await;
        assert!(!has_column(&db, ENTITY_TABLE, ENTITY_REQUIRED_COLUMN)
            .await
            .expect("the probe answers"));

        apply(&db).await.expect("the upgrade applies");

        assert!(has_column(&db, ENTITY_TABLE, ENTITY_REQUIRED_COLUMN)
            .await
            .expect("the probe answers"));
        // An entity of an older daemon was read on every turn, so it
        // reads as required and keeps its behaviour.
        assert!(stored_required(&db).await);
    }

    #[tokio::test]
    async fn the_message_column_keeps_the_episodes_of_an_older_database() {
        let db = old_database().await;
        assert!(!has_column(&db, EPISODE_TABLE, EPISODE_MESSAGE_COLUMN)
            .await
            .expect("the probe answers"));

        apply(&db).await.expect("the upgrade applies");

        assert!(has_column(&db, EPISODE_TABLE, EPISODE_MESSAGE_COLUMN)
            .await
            .expect("the probe answers"));
        // An episode of an older daemon is still read: it names no message,
        // so the transcript marks no turn for it.
        assert_eq!(episode_count(&db).await, (1, 0));
    }

    #[tokio::test]
    async fn the_fact_rating_keeps_the_facts_of_an_older_database() {
        let db = old_database().await;
        assert!(!has_column(&db, EDGE_TABLE, EDGE_CONFIRMED_AT_COLUMN)
            .await
            .expect("the probe answers"));

        apply(&db).await.expect("the upgrade applies");

        assert!(has_column(&db, EDGE_TABLE, EDGE_CONFIRMED_AT_COLUMN)
            .await
            .expect("the probe answers"));
        // A fact of an older daemon reads as one the memory saw once and
        // confirmed when it became true, not when the daemon upgraded, so
        // the seed of a turn reports the age the fact really has.
        assert_eq!(
            stored_fact(&db).await,
            (0, 0.6, "2026-01-01T00:00:00Z".to_string())
        );
    }

    #[tokio::test]
    async fn the_retry_columns_keep_the_episodes_of_an_older_database() {
        let db = old_database().await;
        assert!(!has_column(&db, EPISODE_TABLE, EPISODE_ATTEMPTS_COLUMN)
            .await
            .expect("the probe answers"));

        apply(&db).await.expect("the upgrade applies");

        assert!(has_column(&db, EPISODE_TABLE, EPISODE_ATTEMPTS_COLUMN)
            .await
            .expect("the probe answers"));
        // The episode of an older daemon waits for nobody: the worker
        // reads it on the next pass instead of leaving it behind.
        assert_eq!(stored_attempts(&db).await, (0, 1));
    }

    #[tokio::test]
    async fn the_upgrade_applies_twice_without_a_change() {
        let db = old_database().await;
        apply(&db).await.expect("the upgrade applies");
        apply(&db).await.expect("the second upgrade applies");

        assert!(stored_required(&db).await);
    }
}
