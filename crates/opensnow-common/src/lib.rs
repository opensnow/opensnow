//! # opensnow-common
//!
//! Shared types, error definitions, and configuration primitives used across
//! all OpenSnow crates. Nothing in this crate should depend on any other
//! `opensnow-*` crate — it sits at the bottom of the dependency graph.

pub mod config;
pub mod error;

pub use config::{AuthMode, ServerConfig};
pub use error::{OpenSnowError, Result};
