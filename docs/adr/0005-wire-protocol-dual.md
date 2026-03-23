# ADR 0005 — Wire Protocol: Dual (PostgreSQL + Snowflake REST API v2)

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

The central goal of OpenSnow is **zero-friction migration from Snowflake**.
The wire protocol is the first thing a migrating team hits: if their existing
application code, BI tools, and data pipelines can't connect without changes,
the migration cost is immediately visible and painful.

Two distinct client populations need to be served:

**Population 1 — Generic SQL tooling**  
`psql`, SQLAlchemy, most BI tools (Tableau, Looker, Metabase, Grafana,
Superset), generic JDBC/ODBC drivers, and anything built on the PostgreSQL
ecosystem. These tools speak the **PostgreSQL wire protocol**.

**Population 2 — Snowflake-native tooling**  
`snowflake-connector-python`, the Snowflake JDBC driver, the Go and Node.js
connectors, the `dbt-snowflake` adapter, and `snowsql` itself. These tools
speak Snowflake's proprietary **HTTP REST API v2** (`/api/v2/statements`).

No single protocol satisfies both populations. A PostgreSQL-only approach
would require every team using the official Snowflake connectors to swap out
their connector library — a significant code change, even if the SQL itself
is identical.

---

## Decision

`opensnow-server` implements **both** protocols:

### Protocol 1: PostgreSQL wire protocol (port 5432)

Implemented using the `pgwire` Rust crate. Accepts standard PostgreSQL
connections. Enables the widest possible range of generic tooling with zero
client-side changes for PostgreSQL-compatible tools.

### Protocol 2: Snowflake SQL REST API v2 (port 8080, HTTPS)

Implements Snowflake's documented HTTP REST API exactly:

```
POST /api/v2/statements
GET  /api/v2/statements/{statementHandle}
POST /api/v2/statements/{statementHandle}/cancel
```

Authentication: JWT key-pair (primary) and OAuth2 (secondary), matching
Snowflake's documented auth spec. Response format: Snowflake's `jsonv2`
partition format, matching the exact JSON schema the official connectors
expect.

With this protocol implemented, a team migrating from Snowflake can:

1. Point their account URL from `<account>.snowflakecomputing.com` to their
   OpenSnow cluster hostname.
2. Keep all existing connector code, SQL, and tooling completely unchanged.

---

## Consequences

### Accepted tradeoffs

- **Two protocol implementations to maintain**: bugs or behaviour differences
  must be fixed in both. A shared `QueryDispatcher` layer in `opensnow-server`
  means both protocols funnel into the same execution path — only the
  serialisation layer differs.
- **Snowflake REST API is not a published open standard**: it is documented
  but not formally specified. Behaviour is inferred from the official docs and
  connector source code. Subtle undocumented behaviours may require
  investigation when specific connectors misbehave.
- **HTTPS for REST API v2**: the official connectors require HTTPS. The Helm
  chart handles TLS termination at the ingress layer; local dev uses HTTP
  (the connectors can be configured to skip TLS verification for local use).

### Benefits gained

- **True zero-friction migration**: teams using `snowflake-connector-python`,
  `dbt-snowflake`, or Snowflake JDBC change only the connection string — no
  application code changes required.
- **Broadest possible tooling compatibility**: BI tools, data pipeline
  orchestrators (Airflow, Prefect), and generic SQL clients all work out of
  the box.
- **Async query support**: the REST API v2's `async=true` mode (submit a
  query, poll for results) is a first-class pattern for long-running queries
  and is naturally supported by the statementHandle model.
