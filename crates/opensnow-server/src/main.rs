//! Entry point for the OpenSnow Cloud Services node.
//!
//! This binary starts two listeners:
//! - PostgreSQL wire protocol on port 5432
//! - Snowflake REST API v2 (HTTP) on port 8080
//!
//! Configuration is read from environment variables and/or a config file
//! mounted via Kubernetes ConfigMap (see `deploy/helm/values.yaml`).

fn main() {
    // TODO: initialise tracing/logging
    // TODO: load configuration
    // TODO: connect to catalog (Apache Polaris)
    // TODO: start PostgreSQL wire protocol listener
    // TODO: start Snowflake REST API v2 HTTP server
    println!("opensnow-server starting — not yet implemented");
}
