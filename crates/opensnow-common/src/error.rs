//! Top-level error type for the OpenSnow workspace.

use std::num::ParseIntError;

/// Convenient result alias using [`OpenSnowError`].
pub type Result<T> = std::result::Result<T, OpenSnowError>;

/// Errors that can surface across multiple crates (config, I/O boundaries, etc.).
#[derive(Debug, thiserror::Error)]
pub enum OpenSnowError {
    #[error("configuration: {0}")]
    Config(String),

    #[error("invalid environment variable `{key}`: {source}")]
    InvalidEnvVar {
        key: String,
        #[source]
        source: ParseIntError,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid statement: {0}")]
    InvalidStatement(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("execution error: {0}")]
    Execution(String),
}
