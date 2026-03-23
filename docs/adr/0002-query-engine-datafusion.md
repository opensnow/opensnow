# ADR 0002 — Query Engine: Apache DataFusion

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

A SQL query engine is one of the most complex pieces of systems software to
build correctly. It encompasses:

- SQL parsing and AST construction
- Logical plan building (semantic analysis, type checking, name resolution)
- Cost-based query optimisation (predicate pushdown, join reordering,
  partition pruning)
- Physical plan generation
- Vectorized execution (columnar, SIMD-friendly batch processing)
- Streaming result delivery

Building this from scratch would require 1–2 years of work before any
Snowflake-specific features could be added. The options considered were:

1. **Build a custom engine from scratch** using Apache Arrow for in-memory
   columnar representation
2. **Apache DataFusion** — a Rust-native, extensible SQL query engine built on
   Apache Arrow
3. **Embed DuckDB via FFI** — leverage DuckDB's excellent engine via Rust
   bindings and add Snowflake-specific layers on top

---

## Decision

**Apache DataFusion** is the query execution engine for OpenSnow.

DataFusion provides a complete, production-quality SQL engine as a Rust library
crate. It is designed explicitly to be embedded and extended:

- Custom SQL dialects via `sqlparser-rs` dialect traits
- Custom logical plan nodes for non-standard statements
- Custom physical plan operators
- Custom scalar/aggregate/window UDFs
- Pluggable `TableProvider` and `CatalogProvider` traits — used to wire in
  OpenSnow's Iceberg-backed storage and Polaris catalog
- Pluggable `ObjectStore` trait — used to wire in S3 / MinIO

---

## Consequences

### Accepted tradeoffs

- **DataFusion's SQL dialect is ANSI-based**, not Snowflake-native. All
  Snowflake extensions (VARIANT, QUALIFY, AT/BEFORE, CLONE, UNDROP, etc.)
  must be implemented as custom parser and planner extensions in
  `opensnow-sql`. This is expected and accounted for in the architecture.
- **DataFusion evolves quickly** — major versions can have breaking API
  changes. A pinned workspace dependency and a dedicated upgrade task in the
  backlog mitigates this.
- **Not a drop-in Snowflake execution engine** — DataFusion doesn't understand
  Snowflake's micro-partition format or Iceberg natively; both must be wired
  in via the extension points above.

### Benefits gained

- **1–2 years of engineering time saved** on parser, optimiser, and execution
  engine work that can instead go into Snowflake-specific features.
- **Vectorized execution out of the box**: columnar Arrow-based processing
  with SIMD acceleration gives competitive analytical query performance.
- **Same language**: zero FFI, no C bindings, no separate process — DataFusion
  runs in-process inside `opensnow-compute` as a library.
- **Active Apache community**: DataFusion is used in production by InfluxDB,
  Ballista, Comet (Spark accelerator), and many others — bugs get found and
  fixed rapidly.
- **Arrow ecosystem**: results are naturally in Apache Arrow `RecordBatch`
  format, which maps directly to efficient wire serialisation for both the
  PostgreSQL protocol and the Snowflake REST API JSON format.
