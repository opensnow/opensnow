//! # opensnow-compute
//!
//! Compute layer for OpenSnow. This crate owns the virtual warehouse abstraction
//! and the integration with Apache DataFusion for query execution.
//!
//! ## Virtual Warehouses
//!
//! A virtual warehouse is an isolated pool of DataFusion worker processes. Each
//! warehouse has its own CPU and memory budget; all warehouses share the same
//! underlying S3 storage (read via `opensnow-storage`) and catalog metadata
//! (via `opensnow-catalog`).
//!
//! Warehouse lifecycle:
//!
//! ```text
//! CREATED → STARTING → RUNNING → SUSPENDED → RESUMING → RUNNING
//!                                          ↘ DROPPED
//! ```
//!
//! - `SUSPENDED`: no worker pods running; no compute cost. Auto-suspend fires
//!   after a configurable idle timeout.
//! - `RESUMING`: pods are being scheduled by Kubernetes; queries queue until
//!   ready (auto-resume).
//!
//! ## Query execution flow
//!
//! 1. `opensnow-server` receives a SQL statement and identifies the target
//!    warehouse from the session context.
//! 2. This crate ensures the warehouse is RUNNING (triggering resume if needed).
//! 3. The SQL is handed to `opensnow-sql` for parsing and Snowflake dialect
//!    rewriting, producing a DataFusion `LogicalPlan`.
//! 4. DataFusion executes the plan across the warehouse's worker threads,
//!    reading Parquet micro-partitions from S3 via `opensnow-storage`.
//! 5. Results are streamed back as Arrow `RecordBatch` streams.
//!
//! ## Result Cache
//!
//! The global result cache (keyed on query hash + Iceberg snapshot ID) lives
//! in this crate. A cache hit bypasses DataFusion entirely and returns the
//! previously computed result set.
//!
//! ## Planned modules
//!
//! - `warehouse`  — Warehouse state machine, auto-suspend/resume logic
//! - `manager`    — Warehouse registry, creation/drop, size scaling
//! - `executor`   — DataFusion session context factory, plan execution
//! - `cache`      — Global result cache (in-memory + optional Redis backend)
//! - `scheduler`  — Query queue per warehouse, concurrency slot management

use opensnow_common::{OpenSnowError, Result};
use serde_json::Value;

/// Execute a constrained SQL statement for the initial storage vertical slice.
///
/// Current supported SQL:
/// `SELECT * FROM parquet_scan('<path>') LIMIT <n>`
pub async fn execute_sql(sql: &str) -> Result<Vec<Value>> {
    let parsed = opensnow_sql::parse_select_parquet_scan(sql)?;
    opensnow_storage::read_local_parquet_rows(parsed.path, parsed.limit).map_err(|e| match e {
        OpenSnowError::Storage(_) | OpenSnowError::Io(_) => e,
        other => OpenSnowError::Execution(other.to_string()),
    })
}
