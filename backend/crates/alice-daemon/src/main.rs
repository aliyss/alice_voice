//! Binary entry of the daemon.
//! This file is minimal and delegates to `lifecycle`.

use alice_core::config::CoreConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    init_tracing();
    let config = CoreConfig::from_env();
    if let Err(err) = alice_daemon::lifecycle::run(config).await {
        tracing::error!(error = %err, "daemon failed");
        std::process::exit(1);
    }
}

/// Init tracing with `RUST_LOG` or info default.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
