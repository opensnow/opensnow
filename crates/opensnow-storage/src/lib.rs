//! # opensnow-storage
//!
//! Storage layer for OpenSnow. Responsible for:
//!
//! - Reading and writing Apache Parquet files to/from AWS S3 (and
//!   S3-compatible stores such as MinIO for local development).
//! - Implementing the Apache Iceberg table format on top of Parquet:
//!   micro-partition management, snapshot commits, manifest files, and
//!   metadata JSON.
//! - Exposing a `TableIO` abstraction consumed by `opensnow-compute` so
//!   the query engine can scan and write data without knowing about S3 paths.
//! - Providing column statistics (min/max per micro-partition) for predicate
//!   pushdown and partition pruning in the query planner.
//!
//! ## Storage model
//!
//! Data is organized as follows on S3:
//!
//! ```text
//! s3://<bucket>/
//!   <database>/
//!     <schema>/
//!       <table>/
//!         metadata/
//!           v1.metadata.json       ← Iceberg table metadata
//!           snap-<id>.avro         ← Iceberg snapshot manifest list
//!         data/
//!           <partition>/
//!             <uuid>.parquet       ← Micro-partition data files
//! ```
//!
//! ## Planned modules
//!
//! - `s3`        — `object_store` wrapper for AWS S3 and MinIO
//! - `parquet`   — Parquet reader/writer with Arrow schema integration
//! - `iceberg`   — Iceberg table format: snapshots, manifests, metadata
//! - `stats`     — Column-level statistics extraction for pruning

// TODO: implement
