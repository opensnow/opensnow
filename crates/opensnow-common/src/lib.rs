//! # opensnow-common
//!
//! Shared types, error definitions, and configuration primitives used across
//! all OpenSnow crates. Nothing in this crate should depend on any other
//! opensnow-* crate — it sits at the bottom of the dependency graph.
//!
//! ## Planned contents
//!
//! - `error` — top-level `OpenSnowError` enum and `Result<T>` alias
//! - `config` — shared configuration structs (e.g. S3 coordinates, auth settings)
//! - `types` — common SQL value types, identifiers (database/schema/table names)
//! - `telemetry` — tracing/logging initialisation helpers

// TODO: implement
