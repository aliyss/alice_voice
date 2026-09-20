//! API server of the daemon.
//! This module owns the REST and socket mount.

pub mod conversations;
pub mod dependencies;
pub mod intents;
pub mod openapi;
pub mod reply;
pub mod resolver;
pub mod rest;
pub mod routes;
pub mod settings;
pub mod socket;
pub mod state;

use std::net::SocketAddr;
use std::sync::Arc;

use tracing::info;

use alice_core::config::CoreConfig;

use crate::event_bus::EventBus;
use crate::server::state::{AppState, AppStores};

/// Start the API server on the configured listen address.
pub async fn serve(
    config: Arc<CoreConfig>,
    stores: AppStores,
    events: EventBus,
) -> Result<(), std::io::Error> {
    let addr: SocketAddr = config
        .server
        .listen_addr
        .parse()
        .unwrap_or_else(|_| "0.0.0.0:8787".parse().expect("default addr is valid"));
    let state = AppState::new(config, stores, events);
    let app = routes::build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "daemon listens");
    axum::serve(listener, app).await
}
