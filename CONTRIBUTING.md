# Contributing to OpenSnow

Thank you for your interest in contributing. OpenSnow is an open-source
project and welcomes contributions of all kinds: bug fixes, new features,
documentation improvements, test coverage, and feedback on design decisions.

All contributions are made under the [Apache 2.0 license](LICENSE).

---

## Table of contents

- [Code of conduct](#code-of-conduct)
- [Getting started](#getting-started)
- [Development environment](#development-environment)
- [Project structure](#project-structure)
- [Making changes](#making-changes)
- [Submitting a pull request](#submitting-a-pull-request)
- [Commit message style](#commit-message-style)
- [Running tests](#running-tests)
- [Architecture decisions](#architecture-decisions)

---

## Code of conduct

Be respectful and constructive. We follow the
[Contributor Covenant](https://www.contributor-covenant.org/version/2/1/code_of_conduct/).

---

## Getting started

1. **Fork** the repository on GitHub and clone your fork:

   ```bash
   git clone https://github.com/<your-username>/opensnow.git
   cd opensnow
   ```

2. **Install Rust** (stable toolchain):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup toolchain install stable
   rustup component add rustfmt clippy
   ```

3. **Install Docker and Docker Compose** for the local dev stack:
   - [Docker Desktop](https://www.docker.com/products/docker-desktop/) (macOS/Windows)
   - Or `docker` + `docker compose` plugin on Linux

4. **Start the local stack:**

   ```bash
   docker compose up -d
   ```

   This starts MinIO (S3), PostgreSQL, Apache Polaris (Iceberg catalog),
   and stub containers for the OpenSnow server and worker. See
   `docker-compose.yml` for details.

5. **Check the workspace compiles:**

   ```bash
   cargo check --workspace
   ```

---

## Development environment

### Required tools

| Tool | Version | Install |
|---|---|---|
| Rust | stable (≥ 1.77) | `rustup` |
| Docker | ≥ 24 | docker.com |
| Docker Compose | ≥ 2.20 | bundled with Docker Desktop |
| Helm | ≥ 3.14 | `brew install helm` or [helm.sh](https://helm.sh) |
| Terraform | ≥ 1.6 | `brew install terraform` or [terraform.io](https://terraform.io) |

### Recommended tools

```bash
# cargo-watch — recompile on file save
cargo install cargo-watch

# cargo-nextest — faster test runner
cargo install cargo-nextest

# cargo-deny — license and vulnerability checking
cargo install cargo-deny
```

### Editor setup

VSCode with the `rust-analyzer` extension is recommended. A `.vscode/`
directory with recommended settings will be added in a future PR.

---

## Project structure

```
opensnow/
├── crates/                  ← Rust crates (one concern per crate)
│   ├── opensnow-common/     ← Shared types, errors — no internal deps
│   ├── opensnow-catalog/    ← Iceberg REST catalog client (Polaris)
│   ├── opensnow-storage/    ← S3 + Parquet + Iceberg I/O
│   ├── opensnow-sql/        ← Snowflake SQL dialect (DataFusion extension)
│   ├── opensnow-compute/    ← Virtual warehouse manager + query execution
│   ├── opensnow-auth/       ← JWT, RBAC, session management
│   ├── opensnow-server/     ← Wire protocol servers (binary)
│   └── opensnow-cli/        ← SnowSQL-compatible CLI (binary)
├── deploy/
│   ├── helm/                ← Helm chart
│   └── terraform/aws/       ← AWS Terraform module
├── docs/
│   ├── architecture/        ← Architecture overview
│   └── adr/                 ← Architecture Decision Records
└── tests/
    ├── integration/         ← End-to-end SQL tests
    └── tpch/                ← TPC-H benchmark suite
```

Read [`docs/architecture/overview.md`](docs/architecture/overview.md) before
making significant changes to understand how the layers interact.

---

## Making changes

### Finding something to work on

- Browse [open issues](https://github.com/opensnow/opensnow/issues) for
  `good first issue` and `help wanted` labels.
- Check the roadmap in [`README.md`](README.md) for planned Phase 1 work.
- Open an issue before starting significant new work so the approach can be
  discussed before you invest time writing code.

### Branching

Branch off `main` using a descriptive name:

```bash
git checkout -b feat/storage-s3-parquet-writer
git checkout -b fix/catalog-snapshot-resolution
git checkout -b docs/adr-add-result-cache
```

Prefixes: `feat/`, `fix/`, `docs/`, `test/`, `chore/`, `refactor/`

### Crate boundaries

Each crate has a clear responsibility described in its `src/lib.rs` module
doc comment. Keep dependencies flowing in one direction:

```
opensnow-server / opensnow-cli
  → opensnow-compute → opensnow-sql → opensnow-catalog → opensnow-common
                     → opensnow-storage                 → opensnow-common
  → opensnow-auth                                       → opensnow-common
```

`opensnow-common` must never depend on any other `opensnow-*` crate.

### Code style

- Run `cargo fmt --all` before committing. CI enforces this.
- Run `cargo clippy --workspace --all-features -- -D warnings` and fix all
  warnings before opening a PR.
- Write doc comments (`///`) on all public items.
- Prefer `thiserror` for error types and `anyhow` for error propagation in
  application code. See `opensnow-common` for the project error conventions.

---

## Submitting a pull request

1. Make sure CI passes locally:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-features -- -D warnings
   cargo test --workspace
   helm lint deploy/helm/
   cd deploy/terraform/aws && terraform init -backend=false && terraform validate
   ```

2. Push your branch and open a PR against `main`.

3. Fill in the pull request template — link the related issue, describe what
   changed and why, and confirm you've read the license section.

4. A maintainer will review your PR. Please respond to review comments
   promptly; PRs inactive for 30 days may be closed.

5. PRs are merged with **squash merge** to keep the main branch history clean.

---

## Commit message style

Use the conventional commits format:

```
<type>(<scope>): <short summary>

<optional body — explain the why, not the what>

<optional footer — e.g. Closes #123>
```

**Types:** `feat`, `fix`, `docs`, `test`, `chore`, `refactor`, `perf`  
**Scopes:** crate name or component, e.g. `storage`, `catalog`, `sql`, `helm`

Examples:

```
feat(storage): implement S3 Parquet writer via object_store crate

fix(catalog): handle missing snapshot gracefully in time travel resolution

docs(adr): add ADR for result cache backend choice

chore(deps): upgrade datafusion to 37.0
```

---

## Running tests

```bash
# All unit tests
cargo test --workspace

# A specific crate
cargo test -p opensnow-storage

# Integration tests (requires docker compose up -d first)
cargo test --test integration

# TPC-H benchmark (scale factor 1)
cd tests/tpch && ./run.sh bench --scale-factor 1
```

---

## Architecture decisions

Significant design choices are documented as Architecture Decision Records
in [`docs/adr/`](docs/adr/). If your change introduces a new major dependency,
changes a protocol, or alters the data model, please open a PR that adds or
updates the relevant ADR alongside the code change.

ADR format: copy the structure from any existing ADR — **Status**, **Context**,
**Decision**, **Consequences**.
