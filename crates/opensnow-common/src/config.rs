//! Shared configuration primitives for binaries and libraries.

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Local,
    Oidc,
}

impl Default for AuthMode {
    fn default() -> Self {
        Self::Local
    }
}


use crate::error::{OpenSnowError, Result};

fn parse_u16_env(key: &str, raw: &str) -> Result<u16> {
    raw.parse().map_err(|source| OpenSnowError::InvalidEnvVar {
        key: key.to_string(),
        source,
    })
}

/// Runtime settings for the Cloud Services node (`opensnow-server`).
///
/// Values are loaded from the environment with sensible defaults. Variable
/// names are prefixed with `OPENSNOW_` to avoid collisions with other software
/// on the same host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    /// TCP port for the PostgreSQL wire protocol listener.
    pub pg_port: u16,
    /// TCP port for the Snowflake REST API v2 HTTP listener.
    pub http_port: u16,
    /// Base URL of the Iceberg REST catalog (e.g. Apache Polaris), when set.
    pub polaris_catalog_url: Option<String>,
    /// Bootstrap username used when catalog-backed users are not wired yet.
    pub bootstrap_user: String,
    /// Bootstrap password used when catalog-backed users are not wired yet.
    pub bootstrap_password: String,
    /// HMAC secret used to sign and verify session JWTs.
    pub session_secret: String,
    /// Optional previous secret accepted during signing-key rotation.
    pub previous_session_secret: Option<String>,
    /// Authentication backend mode.
    pub auth_mode: AuthMode,
    /// OIDC JWKS endpoint used to verify bearer tokens in `oidc` mode.
    pub oidc_jwks_url: Option<String>,
    /// Optional OIDC issuer expected in token claims.
    pub oidc_issuer: Option<String>,
    /// Optional OIDC audience expected in token claims.
    pub oidc_audience: Option<String>,
    /// TTL in seconds for cached OIDC JWKS responses.
    pub oidc_jwks_cache_ttl_seconds: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            pg_port: 5432,
            http_port: 8080,
            polaris_catalog_url: None,
            bootstrap_user: "admin".to_string(),
            bootstrap_password: "password".to_string(),
            session_secret: "dev-secret-change-me".to_string(),
            previous_session_secret: None,
            auth_mode: AuthMode::Local,
            oidc_jwks_url: None,
            oidc_issuer: None,
            oidc_audience: None,
            oidc_jwks_cache_ttl_seconds: 300,
        }
    }
}

impl ServerConfig {
    /// Build configuration from `OPENSNOW_*` environment variables, falling
    /// back to [`Default`] for any variable that is unset or empty.
    pub fn from_env() -> Result<Self> {
        let mut cfg = Self::default();

        if let Ok(raw) = std::env::var("OPENSNOW_PG_PORT") {
            if !raw.is_empty() {
                cfg.pg_port = parse_u16_env("OPENSNOW_PG_PORT", &raw)?;
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_HTTP_PORT") {
            if !raw.is_empty() {
                cfg.http_port = parse_u16_env("OPENSNOW_HTTP_PORT", &raw)?;
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_POLARIS_CATALOG_URL") {
            if raw.is_empty() {
                cfg.polaris_catalog_url = None;
            } else {
                cfg.polaris_catalog_url = Some(raw);
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_BOOTSTRAP_USER") {
            if !raw.is_empty() {
                cfg.bootstrap_user = raw;
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_BOOTSTRAP_PASSWORD") {
            if !raw.is_empty() {
                cfg.bootstrap_password = raw;
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_SESSION_SECRET") {
            if !raw.is_empty() {
                cfg.session_secret = raw;
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_PREVIOUS_SESSION_SECRET") {
            if raw.is_empty() {
                cfg.previous_session_secret = None;
            } else {
                cfg.previous_session_secret = Some(raw);
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_AUTH_MODE") {
            if !raw.is_empty() {
                cfg.auth_mode = match raw.to_ascii_lowercase().as_str() {
                    "local" => AuthMode::Local,
                    "oidc" => AuthMode::Oidc,
                    other => {
                        return Err(OpenSnowError::Config(format!(
                            "invalid OPENSNOW_AUTH_MODE `{other}`, expected `local` or `oidc`"
                        )))
                    }
                };
            }
        }
        if let Ok(raw) = std::env::var("OPENSNOW_OIDC_JWKS_URL") {
            cfg.oidc_jwks_url = if raw.is_empty() { None } else { Some(raw) };
        }
        if let Ok(raw) = std::env::var("OPENSNOW_OIDC_ISSUER") {
            cfg.oidc_issuer = if raw.is_empty() { None } else { Some(raw) };
        }
        if let Ok(raw) = std::env::var("OPENSNOW_OIDC_AUDIENCE") {
            cfg.oidc_audience = if raw.is_empty() { None } else { Some(raw) };
        }
        if let Ok(raw) = std::env::var("OPENSNOW_OIDC_JWKS_CACHE_TTL_SECONDS") {
            if !raw.is_empty() {
                cfg.oidc_jwks_cache_ttl_seconds = raw.parse().map_err(|source| {
                    OpenSnowError::InvalidEnvVar {
                        key: "OPENSNOW_OIDC_JWKS_CACHE_TTL_SECONDS".to_string(),
                        source,
                    }
                })?;
            }
        }
        if cfg.auth_mode == AuthMode::Oidc && cfg.oidc_jwks_url.is_none() {
            return Err(OpenSnowError::Config(
                "OPENSNOW_OIDC_JWKS_URL is required when OPENSNOW_AUTH_MODE=oidc".to_string(),
            ));
        }

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ports() {
        let c = ServerConfig::default();
        assert_eq!(c.pg_port, 5432);
        assert_eq!(c.http_port, 8080);
        assert!(c.polaris_catalog_url.is_none());
        assert_eq!(c.bootstrap_user, "admin");
        assert_eq!(c.bootstrap_password, "password");
        assert_eq!(c.session_secret, "dev-secret-change-me");
        assert!(c.previous_session_secret.is_none());
        assert_eq!(c.auth_mode, AuthMode::Local);
        assert!(c.oidc_jwks_url.is_none());
        assert!(c.oidc_issuer.is_none());
        assert!(c.oidc_audience.is_none());
        assert_eq!(c.oidc_jwks_cache_ttl_seconds, 300);
    }
}
