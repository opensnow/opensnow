//! # opensnow-server
//!
//! The Cloud Services node — the entry point for all client connections to
//! OpenSnow. This is the binary that runs as the `cloud-services` Kubernetes
//! Deployment (see `deploy/helm/templates/cloud-services-deployment.yaml`).
//!
//! ## Responsibilities
//!
//! This crate owns the two wire protocols that clients connect over:
//!
//! ### 1. PostgreSQL Wire Protocol (port 5432)
//!
//! Accepts connections from any PostgreSQL-compatible client: `psql`,
//! SQLAlchemy, most BI tools (Tableau, Looker, Metabase, Grafana), and
//! generic ODBC/JDBC drivers. Implemented using the `pgwire` crate.
//!
//! ### 2. Snowflake REST API v2 (port 8080, HTTPS)
//!
//! Implements Snowflake's HTTP REST API endpoints so that official Snowflake
//! drivers (`snowflake-connector-python`, Snowflake JDBC, Go driver, Node.js
//! driver) and the `dbt-snowflake` adapter can connect without any code
//! changes — just point the account URL at the OpenSnow server.
//!
//! Endpoints implemented:
//! - `POST /api/v2/statements`              — submit SQL for execution
//! - `GET  /api/v2/statements/{handle}`     — poll result / stream partitions
//! - `POST /api/v2/statements/{handle}/cancel` — cancel in-flight query
//!
//! Auth for REST API: JWT (key-pair) and OAuth2, matching Snowflake's spec.
//!
//! ### Query dispatch
//!
//! Both protocol handlers funnel into a shared `QueryDispatcher` which:
//! 1. Authenticates and resolves the session context (role, warehouse, db)
//! 2. Hands the SQL to `opensnow-sql` for dialect parsing and rewriting
//! 3. Routes to the appropriate virtual warehouse in `opensnow-compute`
//! 4. Streams results back to the client
//!
//! ## Planned modules
//!
//! - `pg_protocol`   — PostgreSQL wire protocol server (pgwire integration)
//! - `rest_api`      — Axum HTTP router for Snowflake REST API v2
//! - `dispatcher`    — Shared query dispatch and session resolution logic
//! - `result_stream` — Arrow RecordBatch → wire format serialisation

// TODO: implement
