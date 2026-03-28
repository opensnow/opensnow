# SQL Compatibility (Phase 1)

This document tracks what OpenSnow currently understands and what the behavior is for the initial “Phase 1 completion” baseline.

## Execution model (what actually runs)

- SQL is parsed with the Snowflake dialect (`opensnow-sql` via `sqlparser-rs`) to validate that the request is structurally valid.
- Execution is performed by a single embedded DataFusion `SessionContext` (in-memory) with:
  - a legacy fast-path for `SELECT * FROM parquet_scan('<path>') LIMIT <n>`
  - a `COPY INTO ... FILES = ('<local>.parquet')` loader that reads the parquet locally via `opensnow-storage`

## Supported syntax (Phase 1)

### DDL

- `CREATE DATABASE <name>`
- `DROP DATABASE <name> [IF EXISTS]` (custom path; cannot drop DataFusion’s default catalog)
- `CREATE SCHEMA <db>.<schema>`
- `DROP SCHEMA <db>.<schema> [IF EXISTS]` (via DataFusion; non-cascade drops require an empty schema)
- `CREATE TABLE <db>.<schema>.<table> (<col> <type> [, ...])`
- `DROP TABLE <db>.<schema>.<table> [IF EXISTS]` (via DataFusion)

### DML

- `INSERT INTO <db>.<schema>.<table> VALUES (...)`
- `INSERT INTO <db>.<schema>.<table> ...` where the `SELECT` reads from in-memory tables (DataFusion-supported subset)

### SELECT

- Basic `SELECT <exprs> FROM <table> ...` queries over tables created in this embedded session
- `LIMIT`
- simple `ORDER BY` (used for deterministic result assertions in tests)

### Legacy parquet shape (pre-Phase-1 “vertical slice”)

- `SELECT * FROM parquet_scan('<path>') LIMIT <n>`

### `COPY INTO` (Phase 1)

Supported form:

```sql
COPY INTO <db>.<schema>.<table>
FROM @dummy
FILES = ('/absolute/path/file.parquet')
```

Notes:

- Only the `FILES = (...)` local-load path is supported in Phase 1.
- `FROM @stage` credentials/options are ignored.
- The loader currently infers an all-`VARCHAR` schema (all values are stored as strings) from parquet row objects.
- Append/update semantics are not yet implemented; the destination table must not already exist.

## Not supported yet (Phase 1)

- `COPY INTO ... FROM @stage` with remote/object-store stages
- `COPY INTO` without `FILES = (...)`
- Time travel (`AT/BEFORE`, etc.)
- Snowflake-specific extensions like `QUALIFY`, `VARIANT`, `FLATTEN`, etc.
- Persistent catalog/storage; everything in Phase 1 runs in-memory inside the server process.

## PostgreSQL wire protocol baseline (Phase 1)

- OpenSnow exposes a PostgreSQL protocol endpoint that supports:
  - startup/authentication with username/password
  - session lifecycle commands (`SET`, `SHOW`, `BEGIN`, `COMMIT`, `ROLLBACK`)
  - query execution for simple and extended protocol flows (including basic `$n` parameters)
- Current wire-level compatibility is validated with `tokio-postgres` integration smoke tests in
  `crates/opensnow-server/tests/pgwire_smoke.rs`.

