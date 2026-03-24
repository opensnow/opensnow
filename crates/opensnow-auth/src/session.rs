//! Connection-local session context (Snowflake-style `USE` targets).

use serde::{Deserialize, Serialize};

/// Per-connection state carried through the query dispatcher.
///
/// This mirrors Snowflake session attributes that clients set with
/// `USE ROLE`, `USE WAREHOUSE`, `USE DATABASE`, and `USE SCHEMA`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionContext {
    /// Snowflake account identifier (often the hostname or account locator).
    pub account: Option<String>,
    /// Authenticated login name.
    pub user: Option<String>,
    /// Currently active role (`ACCOUNTADMIN`, custom roles, etc.).
    pub role: Option<String>,
    /// Target virtual warehouse for DML and large scans.
    pub warehouse: Option<String>,
    /// Default database for unqualified object names.
    pub database: Option<String>,
    /// Default schema within [`Self::database`].
    pub schema: Option<String>,
}

impl SessionContext {
    /// Returns true if both database and schema are set.
    pub fn has_namespace(&self) -> bool {
        self.database.is_some() && self.schema.is_some()
    }
}
