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

// TODO: implement
