//! Database connection of the daemon.
//! This module owns the SeaORM connection and the schema creation.

use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr, Statement};

use alice_core::config::CoreConfig;

pub mod migrate;

/// Connect to the database, create the schema, and upgrade an older schema.
pub async fn connect(config: &CoreConfig) -> Result<DatabaseConnection, DbErr> {
    let db = Database::connect(&config.database.url).await?;
    init_schema(&db).await?;
    migrate::apply(&db).await?;
    Ok(db)
}

/// Create the tables when they do not exist.
pub(crate) async fn init_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    use sea_orm::DatabaseBackend;

    let sql = match db.get_database_backend() {
        DatabaseBackend::Postgres => {
            r#"
            CREATE TABLE IF NOT EXISTS conversation (
                id UUID PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_conversation_updated_at ON conversation(updated_at);
            CREATE TABLE IF NOT EXISTS message_queue (
                id UUID PRIMARY KEY NOT NULL,
                conversation_id UUID NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                handled_at TIMESTAMPTZ
            );
            CREATE INDEX IF NOT EXISTS idx_message_queue_status ON message_queue(status);
            CREATE TABLE IF NOT EXISTS chat_message (
                id UUID PRIMARY KEY NOT NULL,
                conversation_id UUID NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                intent_id TEXT,
                confidence REAL,
                meta TEXT
            );
            CREATE TABLE IF NOT EXISTS intent (
                id UUID PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL,
                command TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL
            );
            CREATE TABLE IF NOT EXISTS intent_entity (
                id UUID PRIMARY KEY NOT NULL,
                intent_id UUID NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                script TEXT,
                required BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TIMESTAMPTZ NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intent_entity_intent ON intent_entity(intent_id);
            CREATE TABLE IF NOT EXISTS intent_entity_value (
                id UUID PRIMARY KEY NOT NULL,
                entity_id UUID NOT NULL,
                value TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intent_entity_value_entity ON intent_entity_value(entity_id);
            CREATE TABLE IF NOT EXISTS intent_example (
                id UUID PRIMARY KEY NOT NULL,
                intent_id UUID NOT NULL,
                text TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intent_example_intent ON intent_example(intent_id);
            CREATE TABLE IF NOT EXISTS app_setting (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pending_script (
                id UUID PRIMARY KEY NOT NULL,
                conversation_id UUID,
                request_text TEXT NOT NULL,
                summary TEXT NOT NULL,
                script TEXT NOT NULL,
                destructiveness INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_pending_script_status ON pending_script(status);
            CREATE TABLE IF NOT EXISTS memory_node (
                id UUID PRIMARY KEY NOT NULL,
                key TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_node_updated ON memory_node(updated_at);
            CREATE TABLE IF NOT EXISTS memory_edge (
                id UUID PRIMARY KEY NOT NULL,
                subject_id UUID NOT NULL,
                relation TEXT NOT NULL,
                value TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.6,
                importance REAL NOT NULL DEFAULT 0.5,
                confirmations INTEGER NOT NULL DEFAULT 0,
                last_confirmed_at TIMESTAMPTZ NOT NULL,
                valid_at TIMESTAMPTZ NOT NULL,
                invalid_at TIMESTAMPTZ,
                superseded_by UUID,
                source_episode UUID
            );
            CREATE INDEX IF NOT EXISTS idx_memory_edge_subject ON memory_edge(subject_id, relation);
            CREATE TABLE IF NOT EXISTS memory_alias (
                alias TEXT PRIMARY KEY NOT NULL,
                node_id UUID NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_alias_node ON memory_alias(node_id);
            CREATE TABLE IF NOT EXISTS memory_episode (
                id UUID PRIMARY KEY NOT NULL,
                message_id UUID,
                text TEXT NOT NULL,
                reply TEXT NOT NULL,
                intent TEXT,
                processed BOOLEAN NOT NULL DEFAULT FALSE,
                error TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                next_attempt_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_episode_processed ON memory_episode(processed, created_at);
        "#
        }
        _ => {
            r#"
            CREATE TABLE IF NOT EXISTS conversation (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_conversation_updated_at ON conversation(updated_at);
            CREATE TABLE IF NOT EXISTS message_queue (
                id TEXT PRIMARY KEY NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                handled_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_message_queue_status ON message_queue(status);
            CREATE TABLE IF NOT EXISTS chat_message (
                id TEXT PRIMARY KEY NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                created_at TEXT NOT NULL,
                intent_id TEXT,
                confidence REAL,
                meta TEXT
            );
            CREATE TABLE IF NOT EXISTS intent (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL UNIQUE,
                description TEXT NOT NULL,
                command TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS intent_entity (
                id TEXT PRIMARY KEY NOT NULL,
                intent_id TEXT NOT NULL,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                script TEXT,
                required BOOLEAN NOT NULL DEFAULT TRUE,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intent_entity_intent ON intent_entity(intent_id);
            CREATE TABLE IF NOT EXISTS intent_entity_value (
                id TEXT PRIMARY KEY NOT NULL,
                entity_id TEXT NOT NULL,
                value TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intent_entity_value_entity ON intent_entity_value(entity_id);
            CREATE TABLE IF NOT EXISTS intent_example (
                id TEXT PRIMARY KEY NOT NULL,
                intent_id TEXT NOT NULL,
                text TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_intent_example_intent ON intent_example(intent_id);
            CREATE TABLE IF NOT EXISTS app_setting (
                key TEXT PRIMARY KEY NOT NULL,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pending_script (
                id TEXT PRIMARY KEY NOT NULL,
                conversation_id TEXT,
                request_text TEXT NOT NULL,
                summary TEXT NOT NULL,
                script TEXT NOT NULL,
                destructiveness INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_pending_script_status ON pending_script(status);
            CREATE TABLE IF NOT EXISTS memory_node (
                id TEXT PRIMARY KEY NOT NULL,
                key TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_node_updated ON memory_node(updated_at);
            CREATE TABLE IF NOT EXISTS memory_edge (
                id TEXT PRIMARY KEY NOT NULL,
                subject_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                value TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.6,
                importance REAL NOT NULL DEFAULT 0.5,
                confirmations INTEGER NOT NULL DEFAULT 0,
                last_confirmed_at TEXT NOT NULL,
                valid_at TEXT NOT NULL,
                invalid_at TEXT,
                superseded_by TEXT,
                source_episode TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_memory_edge_subject ON memory_edge(subject_id, relation);
            CREATE TABLE IF NOT EXISTS memory_alias (
                alias TEXT PRIMARY KEY NOT NULL,
                node_id TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_alias_node ON memory_alias(node_id);
            CREATE TABLE IF NOT EXISTS memory_episode (
                id TEXT PRIMARY KEY NOT NULL,
                message_id TEXT,
                text TEXT NOT NULL,
                reply TEXT NOT NULL,
                intent TEXT,
                processed BOOLEAN NOT NULL DEFAULT 0,
                error TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                next_attempt_at TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_episode_processed ON memory_episode(processed, created_at);
        "#
        }
    };
    // SeaORM does not support multiple statements in one call for sqlite, so split.
    for stmt in sql.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        db.execute(Statement::from_string(
            db.get_database_backend(),
            stmt.to_string(),
        ))
        .await?;
    }
    Ok(())
}
