# ADR 0004 — Catalog Implementation: Apache Polaris

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

OpenSnow needs a catalog service to track:

- Databases, schemas, tables, views, stages, sequences
- Iceberg table metadata: snapshot history, manifest lists, column statistics,
  partition specs, sort orders
- Virtual warehouse definitions and state
- Time travel retention policies per object
- Zero-copy clone relationships (which snapshot a clone was forked from)
- RBAC: principals, roles, grants, privilege assignments

The catalog must expose an **Iceberg REST Catalog API** so that DataFusion
(via `opensnow-compute`) can resolve table references to concrete Parquet
file paths at query time.

Two options were evaluated in depth:

### Option A: Implement the Iceberg REST Catalog spec from scratch in Rust

Build a custom HTTP service implementing the
[Iceberg REST Catalog OpenAPI spec](https://github.com/apache/iceberg/blob/main/open-api/rest-catalog-open-api.yaml)
backed by PostgreSQL.

**Pros:** Pure Rust stack; full control; no JVM dependency; can co-locate
catalog logic with OpenSnow-specific concepts.

**Cons:** ~30+ REST endpoints to implement correctly; we own all bugs and
spec drift as Iceberg evolves; significant upfront engineering cost that
delays higher-value Snowflake-feature work.

### Option B: Apache Polaris (adopted)

Deploy Apache Polaris as the catalog service. Polaris is an open-source,
Apache-incubating Iceberg REST catalog originally developed by Snowflake and
donated to the Apache Software Foundation in June 2024.

**Pros:** Fully spec-compliant; battle-tested in Snowflake production before
open-sourcing; multi-engine interoperability; RBAC and credential vending
built in; saves months of implementation work.

**Cons:** JVM (Java 21) runtime dependency; still Apache-incubating (API may
shift); less control over internals; another service to operate.

---

## Decision

**Apache Polaris** is the catalog service for OpenSnow (v1).

The JVM dependency is accepted as a known tradeoff. Polaris runs as a
dedicated Kubernetes service (or Docker Compose service in local dev), fully
isolated from the Rust runtime. Users interact with OpenSnow entirely through
the Rust server; Polaris is an internal implementation detail.

If Polaris proves limiting in later phases (e.g. its RBAC model cannot
express Snowflake's full privilege hierarchy, or its API stability is
insufficient), the migration path is to implement a custom Iceberg REST
catalog in Rust that is API-compatible with Polaris — all `opensnow-catalog`
client code continues to work unchanged since it targets the standard
Iceberg REST spec, not Polaris-specific extensions.

---

## Consequences

### Accepted tradeoffs

- **JVM runtime in the stack**: Polaris requires Java 21. Docker Compose and
  Helm chart both include the official `apache/polaris` image. Users deploying
  on Kubernetes need one additional pod per cluster.
- **Incubating project**: Polaris is not yet an Apache top-level project.
  API and behaviour may change between releases. We pin the Polaris version in
  the Helm chart and Docker Compose file and control upgrades explicitly.
- **RBAC mapping**: Polaris has its own principal/role model. A translation
  layer in `opensnow-auth` maps Snowflake RBAC concepts (ACCOUNTADMIN,
  SYSADMIN, custom roles, grants) onto Polaris's model.

### Benefits gained

- **Months of catalog implementation work saved** — redirected to query
  engine, SQL dialect, and Snowflake-feature work.
- **Originally Snowflake's own catalog**: the data model closely mirrors
  Snowflake's internal catalog semantics, minimising conceptual translation.
- **Multi-engine interoperability**: any Iceberg-compatible engine (Spark,
  Trino, Flink, DuckDB) can read OpenSnow tables directly via Polaris, giving
  users freedom to use other tools alongside OpenSnow without data copying.
- **Credential vending**: Polaris can vend short-lived STS credentials for
  S3 access, which is the right security model for production deployments
  (compute nodes never hold long-lived S3 credentials).
- **Standard REST API**: `opensnow-catalog` speaks the Iceberg REST spec, not
  Polaris-specific extensions. Swapping the catalog backend in the future
  requires no changes to the rest of the codebase.
