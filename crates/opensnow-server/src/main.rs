//! Binary entrypoint for OpenSnow Cloud Services.

use opensnow_common::ServerConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = ServerConfig::from_env()?;
    tracing::info!(
        pg_port = config.pg_port,
        http_port = config.http_port,
        polaris_catalog_configured = config.polaris_catalog_url.is_some(),
        "opensnow-server starting (wire protocols not yet implemented)"
    );

    // TODO: connect to catalog (Apache Polaris) when `opensnow-catalog` is wired
    // TODO: start PostgreSQL wire protocol listener (pgwire)
    opensnow_server::run(config).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,opensnow_server=debug,opensnow_common=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}
