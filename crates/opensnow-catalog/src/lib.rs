//! # opensnow-catalog
//!
//! Catalog layer for OpenSnow. Responsible for:
//!
//! - Maintaining the registry of databases, schemas, tables, views, and
//!   virtual warehouses (stored in PostgreSQL via Apache Polaris).
//! - Acting as the Iceberg REST catalog client — all reads and writes to table
//!   metadata go through this crate.
//! - Exposing a `CatalogProvider` trait implementation consumable by
//!   `opensnow-compute` (DataFusion's catalog interface).
//! - Handling time travel metadata: snapshot IDs, data retention periods,
//!   and fail-safe window tracking.
//! - Zero-copy clone bookkeeping: recording clone relationships between
//!   Iceberg snapshots without duplicating storage.
//!
//! ## Dependency on Apache Polaris
//!
//! This crate communicates with a running Apache Polaris instance over its
//! Iceberg REST API (`/api/catalog/v1/...`). Polaris is deployed as a sidecar
//! in the Kubernetes stack (see `deploy/helm/templates/polaris-deployment.yaml`)
//! and as a service in the Docker Compose local dev stack.
//!
//! ## Planned modules
//!
//! - `client`   — HTTP client wrapping the Polaris / Iceberg REST API
//! - `registry` — In-process cache of namespace/table metadata
//! - `warehouse` — Virtual warehouse definitions and state tracking
//! - `snapshot` — Time travel snapshot resolution and retention enforcement

// TODO: implement
