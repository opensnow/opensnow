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

// TODO: implement
