# ADR 0001 — Implementation Language: Rust

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

OpenSnow is a query execution engine and network server. The performance
characteristics of the implementation language directly affect:

- Query throughput and latency (CPU-bound vectorized execution)
- Memory safety under concurrent workloads (multiple warehouses, many connections)
- Operational reliability (crashes in a data warehouse are expensive)
- Ecosystem fit (the leading open-source query engine we plan to build on is
  written in Rust)

The candidates considered were Go, Rust, Java/Kotlin, and Python.

---

## Decision

**Rust** is the implementation language for all OpenSnow server-side crates.

---

## Consequences

### Accepted tradeoffs

- **Steeper contributor onboarding** compared to Go or Python. Mitigated by
  clear crate-level documentation, liberal code comments, and good first-issue
  labelling for contributors new to Rust.
- **Longer initial development velocity** compared to Python or Go while the
  team builds familiarity. Accepted in exchange for the performance and
  correctness guarantees below.
- **Apache Polaris (catalog) is JVM-based** — the Rust-only story is broken at
  the catalog layer. This is a conscious exception; see
  [ADR 0004](0004-catalog-apache-polaris.md).

### Benefits gained

- **Performance**: zero-cost abstractions and no garbage collector mean no GC
  pause jitter during query execution — critical for interactive analytics
  latency percentiles.
- **Memory safety**: the borrow checker eliminates whole classes of bugs
  (use-after-free, data races) that are common failure modes in long-running
  server processes.
- **Apache DataFusion**: the query engine we build on
  ([ADR 0002](0002-query-engine-datafusion.md)) is a first-class Rust crate.
  Using the same language means deep, zero-FFI integration.
- **`object_store` / `arrow` / `parquet` crates**: the Arrow and Parquet
  ecosystem has excellent Rust support, directly usable without bindings.
- **Async runtime (Tokio)**: production-grade async I/O for handling thousands
  of concurrent client connections with low overhead.
- **Single static binary per service**: no runtime installation required in
  container images; images can be built `FROM scratch` or a minimal base.
