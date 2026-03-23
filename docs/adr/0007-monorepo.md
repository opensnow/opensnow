# ADR 0007 — Repository Structure: Monorepo

**Status:** Accepted  
**Date:** 2026-03-23

---

## Context

OpenSnow ships four distinct artifact types that must always be kept in sync:

| Artifact | Technology | Versioned as |
|---|---|---|
| `opensnow-server` container image | Rust binary + Docker | `ghcr.io/opensnow/opensnow-server:vX.Y.Z` |
| `opensnow-worker` container image | Rust binary + Docker | `ghcr.io/opensnow/opensnow-worker:vX.Y.Z` |
| Helm chart | Kubernetes / Helm | `opensnow/opensnow:X.Y.Z` (chart version) |
| Terraform AWS module | HashiCorp Terraform | `github.com/opensnow/opensnow//deploy/terraform/aws?ref=vX.Y.Z` |
| `opensnow` CLI binary | Rust binary | GitHub Release asset |

Two structural options were considered:

### Option A: Monorepo

One repository (`github.com/opensnow/opensnow`) contains all Rust crates,
Dockerfiles, the Helm chart, the Terraform module, and the Docker Compose
local dev file.

### Option B: Polyrepo

Separate repositories per concern:
- `opensnow/opensnow` — Rust source
- `opensnow/helm-charts` — Helm chart
- `opensnow/terraform-aws` — Terraform module

---

## Decision

**Monorepo** (`github.com/opensnow/opensnow`).

---

## Rationale

The decisive factor is **version coupling**. A `v0.4.0` Helm chart must be
tested against `v0.4.0` container images. If the Cloud Services node changes
its configuration environment variables in `v0.5.0`, the Helm chart's
`values.yaml` must change in the same release.

With a monorepo, a single `git tag v0.5.0` triggers CI to:
1. Build and push `opensnow-server:v0.5.0` and `opensnow-worker:v0.5.0`
2. Package and publish Helm chart `0.5.0` with `appVersion: v0.5.0`
3. Tag the Terraform module so `?ref=v0.5.0` resolves correctly
4. Upload CLI binaries to the GitHub Release

Version drift between these artifacts is structurally impossible.

With a polyrepo, a single logical change (e.g. rename an env var) requires
coordinated PRs across multiple repositories, with no atomic way to test
that all three repos at their respective `main` branches work together.

### Additional monorepo benefits

- **Single clone for contributors**: new contributors can read and modify any
  part of the system — Rust code, Helm templates, Terraform, Docker Compose —
  from one `git clone`.
- **Atomic cross-cutting PRs**: a PR that adds a new config option adds it to
  the Rust code, the Helm `values.yaml`, and the Terraform variable in a
  single diff, reviewed together.
- **Shared CI infrastructure**: lint, test, and build rules defined once.
  Path-based filtering (GitHub Actions `paths:`) ensures a pure Terraform
  change doesn't trigger a full Rust build.
- **`docker-compose.yml` co-located with source**: the compose file references
  the code it runs; there is no synchronisation problem.

---

## Consequences

### Accepted tradeoffs

- **Repo grows larger over time**: Rust source, HCL, YAML, Markdown, and
  test fixtures all in one place. Mitigated by a clear top-level directory
  structure and path-filtered CI jobs.
- **Terraform module is not a standalone repo**: users who want to reference
  just the Terraform module point at a subdirectory
  (`//deploy/terraform/aws`). This is standard Terraform practice and works
  without friction.
- **CI pipelines are more complex**: separate jobs for Rust, Helm, and
  Terraform with path-based triggers. Documented in `.github/workflows/`.

### Repository layout

```
opensnow/
├── crates/           ← All Rust crates (Cargo workspace)
├── deploy/
│   ├── helm/         ← Helm chart
│   └── terraform/
│       └── aws/      ← Terraform AWS module
├── docs/             ← Architecture docs and ADRs
├── tests/            ← Integration tests and TPC-H benchmarks
├── Dockerfile.server
├── Dockerfile.worker
├── docker-compose.yml
└── .github/
    └── workflows/    ← CI (ci.yml) and release (release.yml)
```
