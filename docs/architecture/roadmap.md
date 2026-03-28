# OpenSnow Build Roadmap (Current vs Remaining)

This roadmap translates the architecture into an execution view: what is already
implemented in the repository today, and what still needs to be built to reach
the full OpenSnow vision.

Status source:
- `docs/architecture/overview.md`
- `README.md` (`Pre-alpha`, local test constraints)
- `CHANGELOG.md` (`[Unreleased]` entries)

---

## Built Today (Pre-alpha)

Use this document as a living tracker:
- `[x]` complete
- `[~]` partially complete / in progress
- `[ ]` not started

### Platform foundation

- [x] Rust workspace and crate boundaries are in place:
  - [x] `opensnow-common`
  - [x] `opensnow-catalog`
  - [x] `opensnow-storage`
  - [x] `opensnow-sql`
  - [x] `opensnow-compute`
  - [x] `opensnow-auth`
  - [x] `opensnow-server`
  - [x] `opensnow-cli`
- [x] Three-layer architecture is defined and documented (cloud services,
  compute, storage).
- [x] ADR set is complete for core technology choices (language, query engine,
  storage format, catalog, protocols, license, monorepo).

### Runtime and integration scaffolding

- [x] Local stack boots with `docker compose up` (MinIO, Polaris, PostgreSQL,
  server/worker services).
- [x] Deployment scaffolding exists:
  - [x] Helm chart for Kubernetes
  - [x] Terraform module for AWS primitives (EKS/S3/IAM-IRSA)
- [x] Release/CI automation is in place for tests/lint/validate and artifact
  publishing.

### Cloud services and query path (early implementation)

- [x] `opensnow-server` runs and accepts Snowflake REST API v2 requests.
- [x] Authentication/session endpoints exist with local mode plus OIDC mode
  toggles.
- [x] Statement submission + polling flow exists.
- [x] End-to-end SQL works for the Phase 1 in-memory baseline (REST and pgwire):
  - [x] `SELECT * FROM parquet_scan('<absolute_path>') LIMIT <n>` (legacy slice)
  - [x] Qualified DDL/DML, `INSERT ... SELECT`, local `COPY INTO`, and table-backed `SELECT`
- [x] Result response shape and statement lifecycle are implemented at a basic
  level (REST polling + wire row batches).

### What is partially in place but not production-ready

- [~] Dual protocol strategy is defined; PostgreSQL wire baseline is covered by
  smoke tests, but broad client/driver compatibility is not yet proven.
- [~] SQL planning/execution path exists as architecture and crate structure,
  but broad SQL surface, optimization depth, and warehouse orchestration are
  still in progress.
- [~] Catalog/storage model (Iceberg + Polaris + object storage) is selected
  and scaffolded, but full DDL/DML semantics are not complete.

---

## Still To Build

### Phase 1 completion (Foundation)

- [x] Core SQL baseline (single-node, in-memory DataFusion session; see
  `docs/sql-compatibility.md` for exact syntax):
  - [x] `CREATE / DROP DATABASE / SCHEMA / TABLE` (`DROP DATABASE` via custom catalog path; other drops via DataFusion)
  - [x] `INSERT`, `INSERT ... SELECT`, and table-backed `SELECT` (DataFusion subset)
  - [x] `COPY INTO` with local `FILES = ('/abs/path.parquet')` only (no object-store stage integration)
- [x] PostgreSQL wire protocol baseline interoperability for common clients
  (`psql`, SQLAlchemy, BI): connect/auth, session lifecycle, and query execution.
  - [x] Acceptance bar: integration smoke tests cover connect/auth failure,
    session lifecycle (`BEGIN/SET/SHOW/COMMIT/ROLLBACK`), and simple/extended
    query execution in `crates/opensnow-server/tests/pgwire_smoke.rs`.
- [x] Stable single-node execution path beyond `parquet_scan` fixture queries
  (shared embedded `SessionContext`: legacy scan, `COPY INTO`, `DROP DATABASE`, and DataFusion SQL).
- [x] SQL compatibility matrix doc (`docs/sql-compatibility.md`), referenced in
  architecture docs.
- [x] First integration pass for user-facing SQL workflows:
  - [x] REST statement API: DDL/DML/`INSERT ... SELECT`/`SELECT`/`COPY INTO`/teardown
    (`crates/opensnow-server/tests/phase1_sql_baseline.rs`)

### Phase 2 (Warehouses + compatibility)

- [ ] Virtual warehouse management:
  - [ ] `CREATE/ALTER WAREHOUSE`
  - [ ] suspend/resume lifecycle
  - [ ] isolation and routing behavior
- [ ] Snowflake SQL extensions:
  - [ ] `QUALIFY`
  - [ ] `VARIANT` and semi-structured access (`:`, `PARSE_JSON`, `FLATTEN`)
- [ ] Global result cache keyed by SQL/session/snapshot semantics.
- [ ] Practical connector compatibility:
  - [ ] Snowflake Python/JDBC/Node/Go driver paths
  - [ ] `dbt-snowflake`
- [ ] RBAC depth: roles, grants, and role switching behavior parity.

### Phase 3 (Data lifecycle features)

- [ ] Time travel query support:
  - [ ] `AT (TIMESTAMP => ...)`
  - [ ] `AT (OFFSET => ...)`
  - [ ] optional statement-based snapshot references
- [ ] `UNDROP` for table/schema/database.
- [ ] Zero-copy cloning (`CREATE TABLE ... CLONE ...`).
- [ ] Streams and Tasks primitives.
- [ ] Retention/fail-safe lifecycle controls.

### Phase 4 (Hardening + scale)

- [ ] Performance work:
  - [ ] stronger partition pruning and statistics usage
  - [ ] adaptive execution and concurrency tuning
- [ ] Operational maturity:
  - [ ] observability and `ACCOUNT_USAGE`-like views
  - [ ] reliability/SLO posture for long-running deployments
- [ ] Multi-cloud expansion (GCP/Azure object storage and deployment story).
- [ ] Migration tooling to accelerate Snowflake-to-OpenSnow adoption.

---

## Suggested Near-Term Milestones (Next 4-8 Weeks)

- [ ] Expand from fixture-only query shape to core ANSI `SELECT` +
  table-backed reads through Iceberg snapshots.
- [x] Land baseline DDL/DML (`CREATE TABLE`, `INSERT`, `DROP`) with integration
  tests (`phase1_sql_baseline.rs`).
- [x] Publish `docs/sql-compatibility.md` and keep it updated per merged
  feature.
- [x] Validate PostgreSQL wire baseline interoperability with `psql` and one ORM
  end-to-end (connect/auth, session lifecycle, query execution).
- [x] Phase 1 acceptance bar: `phase1_sql_baseline.rs` (REST) +
  `pgwire_smoke.rs` (wire) + `docs/sql-compatibility.md` (documented surface);
  persistence and stages remain out of scope until later phases.
