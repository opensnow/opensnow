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
//! Each connection carries a `SessionContext` containing:
//! - Active role
//! - Active warehouse
//! - Active database and schema
//! - Session-level parameters (e.g. `TIMEZONE`, `QUERY_TAG`)
//!
//! ## Planned modules
//!
//! - `session`   — Session context, parameter store, connection lifecycle
//! - `jwt`       — JWT issuance and verification (key-pair auth)
//! - `password`  — Password hashing and verification
//! - `rbac`      — Role graph, privilege resolution, grant/revoke
//! - `policy`    — Network policies, IP allowlisting

// TODO: implement
