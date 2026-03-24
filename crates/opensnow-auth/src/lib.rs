//! # opensnow-auth
//!
//! Authentication and authorization for OpenSnow. Implements a privilege model
//! that mirrors Snowflake's RBAC system to minimise migration friction.
//!
//! ## Authentication
//!
//! Two mechanisms are supported (matching Snowflake's SQL API):
//!
//! - **Key-pair JWT**: client signs a JWT with their private RSA/EC key;
//!   OpenSnow verifies against the registered public key. This is what the
//!   official Snowflake connectors use by default.
//! - **Username/password**: password is bcrypt-hashed at rest. Used by the
//!   CLI and tools that speak the PostgreSQL wire protocol.
//!
//! ## Role-Based Access Control (RBAC)
//!
//! Mirrors Snowflake's role hierarchy:
//!
//! | Role           | Snowflake equivalent | Default privileges |
//! |----------------|---------------------|--------------------|
//! | `ACCOUNTADMIN` | ACCOUNTADMIN        | Full control        |
//! | `SYSADMIN`     | SYSADMIN            | Create/manage warehouses, databases |
//! | `SECURITYADMIN`| SECURITYADMIN       | Manage users and roles |
//! | `PUBLIC`       | PUBLIC              | Granted to all users automatically |
//!
//! Custom roles can be created with `CREATE ROLE` and granted to users or
//! other roles (`GRANT ROLE r TO USER u`).
//!
//! Privileges follow Snowflake's object hierarchy:
//! `Account > Database > Schema > Table/View/Stage/...`
//!
//! ## Session context
//!
//! Each connection carries a [`SessionContext`] containing:
//! - Active role
//! - Active warehouse
//! - Active database and schema
//! - Session-level parameters (e.g. `TIMEZONE`, `QUERY_TAG`)
//!
//! ## Planned modules
//!
//! - `jwt`       — JWT issuance and verification (key-pair auth)
//! - `rbac`      — Role graph, privilege resolution, grant/revoke
//! - `policy`    — Network policies, IP allowlisting

pub mod password;
pub mod session;
pub mod jwt;
pub mod oidc;

pub use password::{hash_password, verify_password, PasswordError};
pub use jwt::{
    issue_session_token, verify_session_token, verify_session_token_with_secrets, SessionClaims,
    SessionTokenError,
};
pub use session::SessionContext;
pub use oidc::{
    fetch_jwks_json, verify_oidc_access_token, verify_oidc_access_token_with_jwks_json, OidcClaims,
    OidcError,
};
