# ADR 0006 — License: Apache 2.0

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

OpenSnow is an open-source project. The choice of license affects:

- Whether companies can adopt and deploy OpenSnow internally without
  open-sourcing their own code
- Whether companies can build commercial products or managed services on top
  of OpenSnow
- Whether contributors from companies with strict legal policies can
  participate
- The project's relationship with the broader open-source ecosystem

The main candidates were:

| License | Copyleft strength | Notes |
|---|---|---|
| Apache 2.0 | None (permissive) | Patent grant included; enterprise-friendly |
| MIT | None (permissive) | Simple, but no explicit patent grant |
| AGPL-3.0 | Strong (network use triggers) | Prevents closed-source managed services; restricts corporate adoption |
| BSL (Business Source License) | Time-delayed | Used by some data infra projects (CockroachDB, MariaDB); converts to open after a period |

---

## Decision

**Apache License 2.0.**

---

## Rationale

OpenSnow's primary value proposition is making Snowflake-style data
warehousing accessible to teams who want to own their infrastructure. The
project succeeds when it is widely deployed and contributed to. Friction in
adoption and contribution directly undermines this goal.

Apache 2.0 is the most enterprise-friendly open-source license:

- Companies can deploy OpenSnow internally without any license obligations
- Companies can build commercial offerings on top (managed services, SaaS,
  support contracts) without being required to open-source their additions
- The explicit **patent grant** protects contributors and users from patent
  litigation — important for a project in the data infrastructure space
- Contributors from large companies with legal review processes can participate
  without special exceptions
- Compatible with all major open-source dependencies in the stack (DataFusion,
  Iceberg, Arrow, Polaris — all Apache 2.0)

### Why not AGPL?

AGPL's network-use copyleft provision would require any company running a
managed OpenSnow service to open-source their modifications. While this
prevents closed forks, it would also deter adoption by enterprises who cannot
accept copyleft licenses in their internal tools. The project prioritises
broad adoption over preventing commercial use.

---

## Consequences

- The `LICENSE` file at the repo root contains the full Apache 2.0 license text.
- All `Cargo.toml` files carry `license = "Apache-2.0"` in `[workspace.package]`.
- Contributors implicitly grant copyright and patent licenses under Apache 2.0
  by submitting pull requests. A `CONTRIBUTING.md` document makes this explicit.
- Third-party dependencies must be license-compatible with Apache 2.0. A
  `cargo-deny` check in CI enforces this automatically.
