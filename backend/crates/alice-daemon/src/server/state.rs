//! Shared state of the API server.
//! This module holds the app state that the handlers read.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use alice_core::config::CoreConfig;

use crate::conversation::ConversationService;
use crate::event_bus::EventBus;
use crate::execution::CommandRunner;
use crate::handling::HandlingDeps;
use crate::intent::{EntityScripts, IntentService};
use crate::queue::QueueService;
use crate::resolver::gliner::GlinerStore;
use crate::resolver::local::LocalStore;
use crate::resolver::ResolverService;
use crate::settings::SettingsService;

/// The services that the daemon builds at startup.
#[derive(Clone)]
pub struct AppStores {
    /// The connection of the database, so the daemon can report whether
    /// the place that stores every setting still answers.
    pub db: DatabaseConnection,
    /// Conversation service that stores the conversations and their messages.
    pub conversations: ConversationService,
    /// Queue service that stores the messages which wait for handling.
    pub queue: QueueService,
    /// Settings service that stores the values of the settings page.
    pub settings: SettingsService,
    /// Intent service that stores the intent configuration.
    pub intents: IntentService,
    /// Resolver that reads the intent of a message.
    pub resolver: ResolverService,
    /// The live lists of the script entities.
    pub scripts: EntityScripts,
    /// Store of the built in GLiNER models.
    pub gliner_store: Arc<GlinerStore>,
    /// Store of the built in models of the router.
    pub local_store: Arc<LocalStore>,
    /// Runner that runs the command of an intent.
    pub runner: CommandRunner,
}

/// Shared state of the API server.
#[derive(Clone)]
pub struct AppState {
    /// Typed configuration.
    pub config: Arc<CoreConfig>,
    /// The services behind the endpoints.
    pub stores: AppStores,
    /// Time the daemon entered the current state.
    pub started_at: DateTime<Utc>,
    /// Publisher of the socket events.
    pub events: EventBus,
}

impl AppState {
    /// Create a new app state.
    pub fn new(config: Arc<CoreConfig>, stores: AppStores, events: EventBus) -> Self {
        Self {
            config,
            stores,
            started_at: Utc::now(),
            events,
        }
    }

    /// Build the services that handling one message needs.
    pub fn handling_deps(&self) -> HandlingDeps {
        HandlingDeps {
            resolver: self.stores.resolver.clone(),
            runner: self.stores.runner.clone(),
            events: self.events.clone(),
            config: Arc::clone(&self.config),
        }
    }
}
