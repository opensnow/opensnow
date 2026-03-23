# ADR 0003 — Storage Format: Apache Parquet + Apache Iceberg

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

OpenSnow needs a on-disk data format and a table format that together provide:

- Efficient columnar analytical reads (predicate pushdown, column pruning)
- ACID write semantics (concurrent writers don't corrupt each other)
- Snapshot isolation for readers
- A natural implementation of **Time Travel** (query data as of a past point)
- A natural implementation of **Zero-copy cloning** (instant copies with no
  data duplication)
- Schema evolution without rewriting data files
- Long-term interoperability — users who migrate off Snowflake onto OpenSnow
  should be able to also access their data with Spark, Trino, Flink, DuckDB,
  etc., without re-ingestion

The options considered were:

1. **Apache Parquet + Apache Iceberg** (open standard stack)
2. **Custom columnar format** — proprietary micro-partition design, full
   control, zero ecosystem interoperability
3. **Apache Arrow/Feather** — strong in-memory/on-disk story, good Rust
   support, but lacks a mature table format layer for ACID and snapshots

---

## Decision

**Apache Parquet** is the file format for all data files.  
**Apache Iceberg** is the table format that manages those files.

---

## Why Iceberg specifically maps to Snowflake's semantics

Snowflake's storage layer is built around **immutable micro-partitions** —
small, compressed, columnar files that are never modified in place. Writes
produce new files; old files are retained for Time Travel. This is precisely
the Iceberg model:

| Snowflake concept | Iceberg equivalent |
|---|---|
| Micro-partition | Parquet data file tracked in an Iceberg manifest |
| Table version / snapshot | Iceberg snapshot |
| Time Travel (`AT TIMESTAMP`) | Query against a past Iceberg snapshot |
| Fail-safe period | Expired snapshots retained in object storage |
| Zero-copy clone | New Iceberg table referencing the same snapshot |
| Schema evolution | Iceberg schema evolution (add/drop/rename columns) |
| Data retention period | Iceberg snapshot expiry policy |

This alignment means Time Travel and Zero-copy cloning are not bolted-on
features — they fall directly out of Iceberg's snapshot model.

---

## Consequences

### Accepted tradeoffs

- **Iceberg adds operational complexity**: snapshot expiry, manifest
  compaction, and small-file compaction must be run periodically. These are
  well-understood maintenance tasks with tooling support (Spark, Flink, or
  the `iceberg-rust` maintenance APIs).
- **Parquet is not Snowflake's proprietary format**: Snowflake's actual
  micro-partition format is closed-source. OpenSnow will not be byte-for-byte
  compatible with Snowflake's internal file layout — but this is irrelevant to
  users, who interact via SQL.
- **`iceberg-rust` is younger than the Java SDK**: the Rust Iceberg library is
  production-capable but has fewer contributors than the Java reference
  implementation. We track its maturity closely and will contribute upstream
  as needed.

### Benefits gained

- **Open standard**: data files can be read by any Iceberg-compatible engine
  (Spark, Trino, Flink, DuckDB, Dremio) without re-ingestion. Users are never
  locked into OpenSnow.
- **Time Travel and Zero-copy clone are essentially free** to implement — they
  are direct uses of Iceberg's snapshot API.
- **ACID transactions** with optimistic concurrency control — safe for
  multi-warehouse concurrent writes to the same table.
- **Column statistics in manifests** — Iceberg stores per-file min/max
  statistics that DataFusion uses for partition pruning, reducing S3 reads.
- **Apache Polaris compatibility** — Polaris is an Iceberg REST catalog;
  choosing Iceberg as the table format means Polaris is a natural fit for the
  catalog layer ([ADR 0004](0004-catalog-apache-polaris.md)).
