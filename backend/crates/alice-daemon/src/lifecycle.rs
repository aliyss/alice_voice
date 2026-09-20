//! Lifecycle of the daemon.
//! This module wires the config, database, services, and server.

use std::sync::Arc;
use std::time::Duration;

use alice_core::config::CoreConfig;
use chrono::Utc;

use crate::conversation::ConversationService;
use crate::db;
use crate::event_bus::EventBus;
use crate::execution::CommandRunner;
use crate::intent::{defaults, EntityScripts, IntentService};
use crate::queue::QueueService;
use crate::resolver::client::LlamaClient;
use crate::resolver::{GlinerResolver, GlinerStore, LocalEngine, LocalStore, ResolverService};
use crate::server;
use crate::server::state::AppStores;
use crate::settings::SettingsService;

/// Start the daemon and run the server.
pub async fn run(config: CoreConfig) -> Result<(), Box<dyn std::error::Error>> {
    config.validate()?;
    let db = db::connect(&config).await?;

    // 1. Build the services. The defaults of the settings come from the
    //    config, so a database without a settings row still has values.
    let settings = SettingsService::new(db.clone(), SettingsService::defaults(&config));
    let intents = IntentService::new(db.clone());

    // 1b. Seed the example intent into a catalog that holds none, so a
    //     fresh installation answers something.
    match defaults::seed_when_empty(&intents, Utc::now()).await {
        Ok(added) if !added.is_empty() => {
            tracing::info!(intents = %added.join(", "), "the daemon seeded the example intents");
        }
        Ok(_) => {}
        Err(err) => tracing::warn!(error = %err, "the daemon could not seed the example intents"),
    }

    // 2. The scripts of the script entities answer from memory. They run
    //    with a timeout of their own, because a turn waits for the list.
    let scripts = EntityScripts::new(
        CommandRunner::new(
            Duration::from_secs(config.entity_script.timeout_secs),
            config.execution.max_output_bytes,
        ),
        Duration::from_secs(config.entity_script.cache_secs),
        config.entity_script.max_values,
        config.entity_script.label_budget,
    );
    let config = Arc::new(config);

    // 2. Build the built in GLiNER resolver. It shares its store with the
    //    settings page, which downloads and removes the models.
    let gliner_store = Arc::new(GlinerStore::new(config.resolver.gliner.models_dir.clone()));
    // 2b. Build the store and the engine of the built in models of the
    //     router. The settings page downloads the same models.
    let local_store = Arc::new(LocalStore::new(config.resolver.router.models_dir.clone()));
    let local_engine = LocalEngine::new(Arc::clone(&local_store), config.resolver.gliner.threads);
    let resolver = ResolverService::new(
        LlamaClient::new()?,
        GlinerResolver::new(Arc::clone(&gliner_store), config.resolver.gliner.threads),
        local_engine.clone(),
        intents.clone(),
        settings.clone(),
        Arc::clone(&config),
        scripts.clone(),
    );

    // 3. Wire the services and the event bus into the server.
    let (events, _) = EventBus::new();
    let stores = AppStores {
        db: db.clone(),
        conversations: ConversationService::new(db.clone()),
        queue: QueueService::new(db.clone()),
        settings,
        intents,
        resolver,
        scripts,
        gliner_store,
        local_store,
        runner: CommandRunner::new(
            Duration::from_secs(config.execution.timeout_secs),
            config.execution.max_output_bytes,
        ),
    };

    server::serve(config, stores, events).await?;
    Ok(())
}
