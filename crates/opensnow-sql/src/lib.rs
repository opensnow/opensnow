//! # opensnow-sql
//!
//! Snowflake SQL dialect layer for OpenSnow. Sits on top of Apache DataFusion
//! and extends it with Snowflake-specific syntax and semantics.
//!
//! ## Responsibilities
//!
//! - **SQL parsing**: extend DataFusion's SQL parser (via `sqlparser-rs`) to
//!   recognise Snowflake-specific syntax that isn't part of ANSI SQL:
//!   - `AT (TIMESTAMP => ...)` / `BEFORE (STATEMENT => ...)` time travel clauses
//!   - `QUALIFY <window_expr>` clause
//!   - `CREATE TABLE t2 CLONE t1` DDL
//!   - `UNDROP TABLE / SCHEMA / DATABASE`
//!   - `CREATE WAREHOUSE`, `ALTER WAREHOUSE SUSPEND/RESUME`
//!   - `COPY INTO` bulk load syntax
//!   - `:` dot-notation for VARIANT/semi-structured access (`col:key.nested`)
//!
//! - **Type system**: register the `VARIANT` type backed by `serde_json::Value`
//!   and Arrow's `LargeUtf8` for storage.
//!
//! - **Built-in functions**: implement Snowflake-specific scalar and aggregate
//!   functions as DataFusion UDFs:
//!   - Semi-structured: `PARSE_JSON`, `OBJECT_CONSTRUCT`, `ARRAY_AGG`,
//!     `FLATTEN`, `GET_PATH`, `ARRAY_SIZE`
//!   - Date/time: `DATE_TRUNC` (Snowflake signature), `CONVERT_TIMEZONE`,
//!     `DATEADD`, `DATEDIFF`
//!   - String: `LISTAGG`, `SPLIT_PART`, `REGEXP_REPLACE`
//!   - Conditional: `IFF`, `ZEROIFNULL`, `NULLIFZERO`
//!
//! - **Logical plan rewrites**: transform Snowflake-specific logical plan nodes
//!   into DataFusion standard nodes before physical planning:
//!   - Time travel → catalog snapshot lookup
//!   - `QUALIFY` → subquery with `WHERE` on row_number
//!   - `CLONE` → catalog metadata copy operation
//!
//! ## Planned modules
//!
//! - `parser`    — sqlparser-rs dialect + AST extensions
//! - `planner`   — logical plan builder for Snowflake-specific statements
//! - `rewriter`  — AST/logical plan rewrites before DataFusion planning
//! - `functions` — UDF/UDAF registrations
//! - `types`     — VARIANT type definition and Arrow mapping

use opensnow_common::{OpenSnowError, Result};
use sqlparser::dialect::SnowflakeDialect;
use sqlparser::parser::Parser;

/// Re-export for crates that branch on parsed [`Statement`] kinds (e.g. `opensnow-compute`).
pub use sqlparser::ast::{CopyIntoSnowflakeKind, ObjectType, Statement};

/// Constrained statement shape for the first storage vertical slice.
///
/// Supported SQL:
/// `SELECT * FROM parquet_scan('<path>') LIMIT <n>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectParquetScan {
    pub path: String,
    pub limit: usize,
}

/// Parse a minimal `SELECT ... parquet_scan(...) LIMIT ...` statement.
pub fn parse_select_parquet_scan(sql: &str) -> Result<SelectParquetScan> {
    let sql = sql.trim().trim_end_matches(';').trim();
    let sql_lower = sql.to_ascii_lowercase();
    let prefix = "select * from ";
    if !sql_lower.starts_with(prefix) {
        return Err(OpenSnowError::InvalidStatement(
            "only `SELECT * FROM parquet_scan('<path>') LIMIT <n>` is supported in this slice"
                .to_string(),
        ));
    }

    let scan_start = sql_lower.find("parquet_scan(").ok_or_else(|| {
        OpenSnowError::InvalidStatement("missing `parquet_scan(...)` call".to_string())
    })?;
    let arg_start = scan_start + "parquet_scan(".len();
    let arg_end = sql[arg_start..]
        .find(')')
        .map(|idx| arg_start + idx)
        .ok_or_else(|| OpenSnowError::InvalidStatement("missing `)` after parquet_scan".to_string()))?;

    let raw_arg = sql[arg_start..arg_end].trim();
    if !(raw_arg.starts_with('\'') && raw_arg.ends_with('\'')) || raw_arg.len() < 2 {
        return Err(OpenSnowError::InvalidStatement(
            "parquet_scan argument must be single-quoted path".to_string(),
        ));
    }
    let path = raw_arg[1..raw_arg.len() - 1].to_string();
    if path.is_empty() {
        return Err(OpenSnowError::InvalidStatement(
            "parquet_scan path must not be empty".to_string(),
        ));
    }

    let trailing = sql[arg_end + 1..].trim();
    let trailing_lower = trailing.to_ascii_lowercase();
    let limit_prefix = "limit ";
    if !trailing_lower.starts_with(limit_prefix) {
        return Err(OpenSnowError::InvalidStatement(
            "statement must end with `LIMIT <n>`".to_string(),
        ));
    }
    let limit = trailing[limit_prefix.len()..]
        .trim()
        .parse::<usize>()
        .map_err(|_| OpenSnowError::InvalidStatement("limit must be a positive integer".to_string()))?;

    Ok(SelectParquetScan { path, limit })
}

/// Parse exactly one statement using the Snowflake SQL dialect (sqlparser).
pub fn parse_one_statement(sql: &str) -> Result<Statement> {
    let sql = sql.trim().trim_end_matches(';').trim();
    let dialect = SnowflakeDialect {};
    let mut stmts = Parser::parse_sql(&dialect, sql)
        .map_err(|e| OpenSnowError::InvalidStatement(e.to_string()))?;
    if stmts.is_empty() {
        return Err(OpenSnowError::InvalidStatement("empty statement".into()));
    }
    if stmts.len() > 1 {
        return Err(OpenSnowError::InvalidStatement(
            "exactly one SQL statement is supported".into(),
        ));
    }
    Ok(stmts.remove(0))
}

/// Accept legacy `parquet_scan` queries or any statement valid for [`parse_one_statement`].
pub fn validate_statement(sql: &str) -> Result<()> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err(OpenSnowError::InvalidStatement("empty statement".into()));
    }
    if parse_select_parquet_scan(sql).is_ok() {
        return Ok(());
    }
    parse_one_statement(sql).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_shape() {
        let parsed =
            parse_select_parquet_scan("SELECT * FROM parquet_scan('/tmp/a.parquet') LIMIT 10")
                .unwrap();
        assert_eq!(parsed.path, "/tmp/a.parquet");
        assert_eq!(parsed.limit, 10);
    }

    #[test]
    fn rejects_invalid_shape() {
        let err = parse_select_parquet_scan("SELECT 1").unwrap_err();
        assert!(matches!(err, OpenSnowError::InvalidStatement(_)));
    }

    #[test]
    fn validate_accepts_select_and_ddl() {
        validate_statement("SELECT 1").unwrap();
        validate_statement("CREATE DATABASE db1").unwrap();
        validate_statement("CREATE SCHEMA s1").unwrap();
    }

    #[test]
    fn validate_rejects_garbage() {
        assert!(validate_statement(";;;").is_err());
    }
}
