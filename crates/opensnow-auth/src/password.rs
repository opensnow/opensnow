//! Username/password credentials using bcrypt at rest.

use thiserror::Error;

/// Cost factor for bcrypt (work factor). Higher is slower and more resistant
/// to brute force; 12 is a reasonable default for server-side hashing.
const DEFAULT_COST: u32 = 12;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password hashing failed")]
    Hash,

    #[error("password verification failed")]
    Verify,
}

/// Produce a bcrypt hash suitable for persistence (e.g. catalog user row).
pub fn hash_password(password: impl AsRef<[u8]>) -> Result<String, PasswordError> {
    bcrypt::hash(password.as_ref(), DEFAULT_COST).map_err(|_| PasswordError::Hash)
}

/// Check a plaintext password against a stored bcrypt hash.
pub fn verify_password(password: impl AsRef<[u8]>, hash: &str) -> Result<bool, PasswordError> {
    bcrypt::verify(password.as_ref(), hash).map_err(|_| PasswordError::Verify)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let h = hash_password("correct-horse-battery-staple").unwrap();
        assert!(verify_password("correct-horse-battery-staple", &h).unwrap());
        assert!(!verify_password("wrong", &h).unwrap());
    }
}
