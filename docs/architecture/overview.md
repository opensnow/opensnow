# OpenSnow Architecture Overview

## What is OpenSnow?

OpenSnow is an open-source, self-hosted data warehouse that replicates Snowflake's
architecture and SQL dialect. Teams can deploy OpenSnow on their own AWS infrastructure
(multi-cloud in future releases) and run the same SnowSQL queries they write today for
Snowflake — with no retraining and minimal migration friction.

---

## Three-Layer Architecture

OpenSnow mirrors Snowflake's three-layer separation of concerns: storage, compute, and
cloud services operate independently and scale independently.

```
  Clients
  (psql · dbt · snowflake-connector-python · Tableau · BI tools)
         │                        │
         │ PostgreSQL              │ Snowflake REST API v2
         │ wire protocol           │ POST /api/v2/statements
         ▼                        ▼
┌─────────────────────────────────────────────────────────────────┐
│                    CLOUD SERVICES LAYER                         │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────────┐   │
│  │     Auth     │  │    Query     │  │  Session & RBAC    │   │
│  │  (JWT/OAuth) │  │   Planner    │  │  (roles, grants,   │   │
│  │  opensnow-   │  │  (DataFusion │  │   USE ROLE/WH/DB)  │   │
│  │     auth     │  │   LogicalPlan│  │  opensnow-auth     │   │
│  └──────────────┘  └──────────────┘  └────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │               Result Cache                               │  │
│  │  (keyed on query hash + Iceberg snapshot ID)             │  │
│  │  Cache hit → return immediately, bypass compute          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │           Catalog Client (Apache Polaris)                │  │
│  │  Databases · Schemas · Tables · Snapshots · Warehouses   │  │
│  │  opensnow-catalog  ←→  Polaris Iceberg REST API          │  │
│  └──────────────────────────────────────────────────────────┘  │
│                         opensnow-server                         │
└───────────────────────────┬─────────────────────────────────────┘
                            │  dispatches to warehouse
          ┌─────────────────┼─────────────────┐
          ▼                 ▼                 ▼
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  Warehouse   │  │  Warehouse   │  │  Warehouse   │
│  ANALYTICS   │  │  ETL_WH      │  │  ADHOC_WH    │
│  (Large)     │  │  (Medium)    │  │  (Small)     │
│              │  │              │  │              │
│  DataFusion  │  │  DataFusion  │  │  DataFusion  │
│  workers     │  │  workers     │  │  workers     │
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │                 │                 │
       └─────────────────┴─────────────────┘
                         │  reads/writes Parquet via Iceberg
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                       STORAGE LAYER                             │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Apache Polaris                        │   │
│  │           Iceberg REST Catalog (metadata)               │   │
│  │  Namespaces · Table snapshots · Manifest lists ·        │   │
│  │  Column statistics · Retention policies                 │   │
│  │  Backed by PostgreSQL                                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │          AWS S3 (MinIO in local dev)                    │   │
│  │                                                         │   │
│  │  s3://bucket/<db>/<schema>/<table>/                     │   │
│  │    metadata/  ← Iceberg metadata JSON + manifests       │   │
│  │    data/      ← Parquet micro-partition files           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                       opensnow-storage                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Components

### Cloud Services Layer (`opensnow-server`)

The entry point for all client traffic. Runs as the `cloud-services` Kubernetes
Deployment. Owns two wire protocols:

| Protocol | Port | Enables |
|---|---|---|
| PostgreSQL wire protocol | 5432 | `psql`, SQLAlchemy, JDBC/ODBC, BI tools (Tableau, Looker, Metabase) |
| Snowflake REST API v2 | 8080 | Official Snowflake connectors (`snowflake-connector-python`, JDBC, Go, Node.js), `dbt-snowflake` adapter |

Both protocols funnel into a shared `QueryDispatcher` which authenticates the session,
resolves the active role and warehouse, parses the SQL (via `opensnow-sql`), checks the
result cache, and dispatches to the appropriate virtual warehouse.

### Compute Layer (`opensnow-compute`)

Each **virtual warehouse** is an isolated pool of [Apache DataFusion] worker processes
sharing no resources with other warehouses. All warehouses read from and write to the same
S3 storage layer — only compute is isolated.

Warehouse lifecycle:

```
CREATED → STARTING → RUNNING ⇄ SUSPENDED
                             ↘ DROPPED
```

- **Auto-suspend**: idle warehouses are suspended after a configurable timeout (no
  compute cost while suspended).
- **Auto-resume**: the first query to arrive against a suspended warehouse triggers
  automatic resumption before execution.
- **Multi-cluster**: a single warehouse can run multiple DataFusion clusters to absorb
  concurrency spikes (Phase 2).

### Storage Layer (`opensnow-storage` + Apache Polaris)

All data is stored as **Apache Parquet** micro-partition files on S3, managed by the
**Apache Iceberg** table format. The Iceberg catalog is served by **Apache Polaris** — an
open-source Iceberg REST catalog originally developed by Snowflake.

Iceberg provides:
- **ACID transactions** — concurrent writes without corruption
- **Snapshot isolation** — readers always see a consistent version of the data
- **Time travel** — every write produces a new snapshot; past snapshots are queryable via
  `AT (TIMESTAMP => ...)` and retained per the table's `DATA_RETENTION_TIME_IN_DAYS` policy
- **Zero-copy cloning** — a clone is a new Iceberg table that initially references the
  same snapshot files as the source; no data is copied until diverging writes occur
- **Schema evolution** — add, rename, drop columns without rewriting data files

---

## Query Lifecycle

A full round-trip from SQL to result:

```
1.  Client sends SQL over PostgreSQL wire or Snowflake REST API v2
        │
2.  opensnow-server receives the request
        │
3.  opensnow-auth authenticates (JWT or username/password) and resolves
    the session: active role, warehouse, database, schema
        │
4.  Result cache check: hash(SQL + session params + current snapshot ID)
    ├─ HIT  → return cached result immediately (no compute used)
    └─ MISS → continue
        │
5.  opensnow-sql parses the SQL using the Snowflake dialect extension
    (sqlparser-rs) and produces a DataFusion LogicalPlan.
    Snowflake-specific nodes (AT/BEFORE, QUALIFY, CLONE, UNDROP) are
    rewritten into standard DataFusion nodes or catalog operations.
        │
6.  opensnow-catalog resolves table references to Iceberg snapshots
    (applying time travel if AT/BEFORE is present).
        │
7.  The LogicalPlan is dispatched to the target virtual warehouse.
    If the warehouse is SUSPENDED, auto-resume is triggered first.
        │
8.  DataFusion optimises the plan (predicate pushdown, partition pruning
    using Iceberg column statistics, join reordering).
        │
9.  DataFusion workers execute the physical plan, reading Parquet
    micro-partitions from S3 via opensnow-storage.
        │
10. Results are streamed as Arrow RecordBatches back to opensnow-server.
        │
11. opensnow-server serialises results into the wire format
    (PostgreSQL DataRow messages or Snowflake REST API JSON partitions)
    and streams them to the client.
        │
12. Result is stored in the result cache.
```

---

## Snowflake SQL Dialect (`opensnow-sql`)

OpenSnow targets the **core Snowflake SQL dialect** — ANSI SQL plus the Snowflake
extensions most commonly used in production workloads:

| Feature | SQL syntax | Status |
|---|---|---|
| VARIANT / semi-structured | `col:key.nested`, `PARSE_JSON()`, `FLATTEN()` | Planned (Phase 2) |
| Time Travel | `AT (TIMESTAMP => ...)`, `AT (OFFSET => -3600)`, `AT (STATEMENT => '<id>')` | Planned (Phase 3) |
| Zero-copy clone | `CREATE TABLE t2 CLONE t1` | Planned (Phase 3) |
| QUALIFY clause | `SELECT ... QUALIFY ROW_NUMBER() OVER (...) = 1` | Planned (Phase 2) |
| UNDROP | `UNDROP TABLE / SCHEMA / DATABASE` | Planned (Phase 3) |
| Virtual warehouses | `CREATE WAREHOUSE`, `ALTER WAREHOUSE SUSPEND/RESUME` | Planned (Phase 2) |
| COPY INTO | `COPY INTO table FROM @stage` | Planned (Phase 1) |
| Streams & Tasks | `CREATE STREAM`, `CREATE TASK` | Planned (Phase 3) |

A full SQL compatibility matrix is maintained in `docs/sql-compatibility.md` (to be
created during Phase 1 implementation).

---

## Deployment Model

### Local Development

```bash
docker compose up
```

Starts the full stack locally using MinIO instead of S3. No cloud account required.
See `docker-compose.yml` for service details.

### Production (Kubernetes / AWS)

Two steps:

**Step 1 — Provision AWS infrastructure** (EKS cluster, S3 bucket, IAM roles):

```bash
cd deploy/terraform/aws
terraform init && terraform apply
```

**Step 2 — Deploy OpenSnow via Helm**:

```bash
helm repo add opensnow https://charts.opensnow.io
helm install opensnow opensnow/opensnow -f my-values.yaml -n opensnow
```

### Artifact model

Every GitHub release (`git tag vX.Y.Z`) produces four artifacts, all at the same version:

| Artifact | Location | Consumed by |
|---|---|---|
| `opensnow-server` container image | `ghcr.io/opensnow/opensnow-server:vX.Y.Z` | Kubernetes / docker compose |
| `opensnow-worker` container image | `ghcr.io/opensnow/opensnow-worker:vX.Y.Z` | Kubernetes / docker compose |
| Helm chart | `https://charts.opensnow.io` (GitHub Pages) | `helm install` |
| `opensnow` CLI binary | GitHub Release assets (macOS/Linux/Windows) | End users, CI scripts |

The Helm chart's `appVersion` is always kept in sync with the container image tags by the
release CI workflow (`.github/workflows/release.yml`).

---

## Crate Dependency Graph

```
opensnow-cli ──────────────────────────────────────────► opensnow-common
opensnow-server ──► opensnow-compute ──► opensnow-sql ──► opensnow-catalog ──► opensnow-common
                │                   └──► opensnow-storage ──────────────────► opensnow-common
                ├──► opensnow-auth ──────────────────────────────────────────► opensnow-common
                └──► opensnow-catalog
```

`opensnow-common` has no dependencies on other opensnow-* crates and sits at the bottom
of the graph. This prevents circular dependencies and keeps shared types easy to evolve.

---

## Further Reading

- [`docs/adr/`](../adr/) — Architecture Decision Records for every major technology choice
- [`deploy/helm/values.yaml`](../../deploy/helm/values.yaml) — all configuration options
- [`deploy/terraform/aws/`](../../deploy/terraform/aws/) — AWS infrastructure provisioning
- [`docker-compose.yml`](../../docker-compose.yml) — local development stack
