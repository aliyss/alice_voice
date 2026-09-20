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

/// Apply every upgrade that the given database needs.
pub async fn apply(db: &DatabaseConnection) -> Result<(), DbErr> {
    migrate_message_conversation(db).await?;
    migrate_message_meta(db).await?;
    migrate_entity_script(db).await?;
    migrate_entity_required(db).await?;
    create_message_index(db).await
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
    async fn the_upgrade_applies_twice_without_a_change() {
        let db = old_database().await;
        apply(&db).await.expect("the upgrade applies");
        apply(&db).await.expect("the second upgrade applies");

        assert!(stored_required(&db).await);
    }
}
