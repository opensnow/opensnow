//! Signed JWT session tokens used by the REST API.

use crate::SessionContext;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionClaims {
    pub sub: String,
    pub account: Option<String>,
    pub role: Option<String>,
    pub warehouse: Option<String>,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
}

#[derive(Debug, Error)]
pub enum SessionTokenError {
    #[error("session token creation failed")]
    Encode,
    #[error("session token verification failed")]
    Decode,
    #[error("session context missing user")]
    MissingUser,
}

pub fn issue_session_token(
    secret: &str,
    session: &SessionContext,
    now_epoch_secs: u64,
    ttl_secs: u64,
) -> Result<String, SessionTokenError> {
    let Some(sub) = session.user.clone() else {
        return Err(SessionTokenError::MissingUser);
    };
    let claims = SessionClaims {
        sub,
        account: session.account.clone(),
        role: session.role.clone(),
        warehouse: session.warehouse.clone(),
        database: session.database.clone(),
        schema: session.schema.clone(),
        iat: now_epoch_secs,
        exp: now_epoch_secs.saturating_add(ttl_secs),
        jti: Uuid::new_v4().to_string(),
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| SessionTokenError::Encode)
}

pub fn verify_session_token(secret: &str, token: &str) -> Result<SessionClaims, SessionTokenError> {
    let mut validation = Validation::default();
    validation.leeway = 0;
    decode::<SessionClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|decoded| decoded.claims)
    .map_err(|_| SessionTokenError::Decode)
}

pub fn verify_session_token_with_secrets(
    secrets: &[String],
    token: &str,
) -> Result<SessionClaims, SessionTokenError> {
    for secret in secrets {
        if let Ok(claims) = verify_session_token(secret, token) {
            return Ok(claims);
        }
    }
    Err(SessionTokenError::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_with_rotated_previous_secret() {
        let session = SessionContext {
            account: Some("local".to_string()),
            user: Some("admin".to_string()),
            role: Some("PUBLIC".to_string()),
            warehouse: None,
            database: None,
            schema: None,
        };
        let token = issue_session_token("old-secret", &session, 2_000_000_000, 3600).unwrap();
        let claims =
            verify_session_token_with_secrets(&["new-secret".to_string(), "old-secret".to_string()], &token)
                .unwrap();
        assert_eq!(claims.sub, "admin");
    }
}

