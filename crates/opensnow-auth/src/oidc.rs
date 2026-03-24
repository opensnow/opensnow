//! OIDC access-token verification against JWKS.

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct OidcClaims {
    pub sub: String,
    pub preferred_username: Option<String>,
    pub iss: Option<String>,
    pub exp: u64,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    kty: String,
    alg: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Debug, Error)]
pub enum OidcError {
    #[error("token header is invalid")]
    InvalidHeader,
    #[error("failed to fetch jwks")]
    Fetch,
    #[error("jwks did not contain matching rsa key")]
    MissingKey,
    #[error("token verification failed")]
    Verify,
}

pub async fn verify_oidc_access_token(
    token: &str,
    jwks_url: &str,
    issuer: Option<&str>,
    audience: Option<&str>,
) -> Result<OidcClaims, OidcError> {
    let jwks_json = fetch_jwks_json(jwks_url).await?;
    verify_oidc_access_token_with_jwks_json(token, &jwks_json, issuer, audience)
}

pub async fn fetch_jwks_json(jwks_url: &str) -> Result<String, OidcError> {
    reqwest::get(jwks_url)
        .await
        .map_err(|_| OidcError::Fetch)?
        .text()
        .await
        .map_err(|_| OidcError::Fetch)
}

pub fn verify_oidc_access_token_with_jwks_json(
    token: &str,
    jwks_json: &str,
    issuer: Option<&str>,
    audience: Option<&str>,
) -> Result<OidcClaims, OidcError> {
    let header = decode_header(token).map_err(|_| OidcError::InvalidHeader)?;
    let jwks: Jwks = serde_json::from_str(jwks_json).map_err(|_| OidcError::Fetch)?;
    let key = jwks
        .keys
        .iter()
        .find(|k| {
            k.kty == "RSA"
                && k.n.is_some()
                && k.e.is_some()
                && match (&header.kid, &k.kid) {
                    (Some(expected), Some(actual)) => expected == actual,
                    (Some(_), None) => false,
                    (None, _) => true,
                }
        })
        .ok_or(OidcError::MissingKey)?;

    let algorithm = match key.alg.as_deref() {
        Some("RS256") | None => Algorithm::RS256,
        Some("RS384") => Algorithm::RS384,
        Some("RS512") => Algorithm::RS512,
        _ => Algorithm::RS256,
    };
    let mut validation = Validation::new(algorithm);
    validation.leeway = 0;
    if let Some(iss) = issuer {
        validation.set_issuer(&[iss]);
    }
    if let Some(aud) = audience {
        validation.set_audience(&[aud]);
    }

    let decoding_key = DecodingKey::from_rsa_components(
        key.n.as_deref().ok_or(OidcError::MissingKey)?,
        key.e.as_deref().ok_or(OidcError::MissingKey)?,
    )
    .map_err(|_| OidcError::MissingKey)?;

    decode::<OidcClaims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|_| OidcError::Verify)
}

