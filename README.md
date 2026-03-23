# OpenSnow

Open-source, self-hosted data warehouse compatible with the Snowflake SQL
dialect and official Snowflake drivers. Deploy on your own AWS infrastructure
and run the same SnowSQL queries you write today — no retraining, no
migration tax.

> **Status:** Pre-alpha — scaffolding and architecture complete, implementation
> in progress. Not yet suitable for production use.

---

## What is OpenSnow?

OpenSnow replicates Snowflake's three-layer architecture on infrastructure you
own and operate:

- **Storage layer** — Apache Parquet files on AWS S3, managed by Apache
  Iceberg. Provides ACID transactions, time travel, and zero-copy cloning.
- **Compute layer** — Independent virtual warehouses backed by Apache
  DataFusion. Each warehouse is an isolated compute pool; all share the same
  storage.
- **Cloud Services layer** — Authentication, query planning, result caching,
  catalog integration. Exposes both the PostgreSQL wire protocol and the
  Snowflake REST API v2 so existing tooling connects without changes.

## What OpenSnow is not

- A managed service. You run it; you own it.
- A 100% Snowflake feature replica on day one. We target the core SQL dialect
  and the highest-value features first (see roadmap below).
- A replacement for Snowflake's proprietary AI/ML features (Cortex, etc.) in
  the near term.

---

## Quickstart — local development

Requires Docker and Docker Compose.

```bash
git clone https://github.com/opensnow/opensnow
cd opensnow
docker compose up
```

This starts:
- **MinIO** at `http://localhost:9001` (S3-compatible storage, user: `minioadmin`)
- **Apache Polaris** at `http://localhost:8181` (Iceberg REST catalog)
- **PostgreSQL** at `localhost:5433` (catalog metadata)
- **opensnow-server** at `localhost:5432` (PostgreSQL wire) and `localhost:8080` (Snowflake REST API v2)

Connect with the CLI:

```bash
opensnow --account localhost --user admin --password password
```

Or with any PostgreSQL client:

```bash
psql -h localhost -p 5432 -U admin
```

Or point your existing Snowflake connector at `localhost:8080`.

---

## Production deployment (AWS)

**Step 1 — Provision AWS infrastructure:**

```bash
cd deploy/terraform/aws
cp terraform.tfvars.example terraform.tfvars
# edit terraform.tfvars (set your S3 bucket name and passwords)
terraform init && terraform apply
```

This creates an EKS cluster, S3 bucket, and IAM roles for IRSA.

**Step 2 — Deploy via Helm:**

```bash
helm repo add opensnow https://charts.opensnow.io
helm install opensnow opensnow/opensnow \
  --namespace opensnow --create-namespace \
  -f my-values.yaml
```

See [`deploy/helm/values.yaml`](deploy/helm/values.yaml) for all configuration
options.

---

## Documentation Site

The documentation site is built with [Astro](https://astro.build) and deployed to
[Netlify](https://netlify.com).

**Local development:**

```bash
cd site
npm install
npm run dev
```

**Deployment:**

The site is automatically deployed on release tags via GitHub Actions. You'll need
to configure the following secrets in your repository:

- `NETLIFY_AUTH_TOKEN` - A Netlify personal access token
- `NETLIFY_SITE_ID` - The ID of your Netlify site

The Helm chart is packaged and served at `/charts/` on the deployed site.

---

## Migrating from Snowflake

The goal is zero-friction migration. Teams should be able to:

1. Point their Snowflake connector's account URL at the OpenSnow server.
2. Keep all existing SQL, connection code, and tooling unchanged.

**Supported connection methods:**

| Method | Status |
|---|---|
| `snowflake-connector-python` | Planned (Phase 2) |
| `dbt-snowflake` adapter | Planned (Phase 2) |
| Snowflake JDBC driver | Planned (Phase 2) |
| `psql` / SQLAlchemy / generic JDBC | Planned (Phase 1) |
| Tableau / Looker / Metabase | Planned (Phase 1, via PostgreSQL wire) |

---

## Roadmap

### Phase 1 — Foundation (Months 1–3)
- Single-node deployment, basic analytical SQL
- `CREATE / DROP DATABASE / SCHEMA / TABLE`, `INSERT`, `SELECT`, `COPY INTO`
- PostgreSQL wire protocol server
- S3 + Iceberg storage layer
- Apache Polaris catalog integration
- `docker compose up` local dev experience

### Phase 2 — Virtual Warehouses + Snowflake Extensions (Months 4–6)
- Multiple isolated virtual warehouses (`CREATE WAREHOUSE`, auto-suspend/resume)
- `VARIANT` type and semi-structured data (`:` notation, `PARSE_JSON`, `FLATTEN`)
- `QUALIFY` clause
- Global result cache
- Snowflake REST API v2 (`/api/v2/statements`) — official connector compatibility
- RBAC (roles, grants, `USE ROLE`)

### Phase 3 — Time Travel + Zero-Copy Cloning (Months 7–9)
- `AT (TIMESTAMP => ...)` / `AT (OFFSET => ...)` time travel queries
- `UNDROP TABLE / SCHEMA / DATABASE`
- `CREATE TABLE t2 CLONE t1` zero-copy cloning
- Streams and Tasks (CDC + scheduled SQL)
- Fail-safe retention period

### Phase 4 — Hardening + Multi-Cloud (Months 10–12+)
- Query performance optimisation (micro-partition pruning, adaptive execution)
- GCP and Azure storage support
- Snowflake schema migration tool
- `ACCOUNT_USAGE` observability views

---

## Architecture

See [`docs/architecture/overview.md`](docs/architecture/overview.md) for the
full three-layer architecture diagram, query lifecycle, and component
descriptions.

Architecture Decision Records (the "why" behind every major technology
choice) are in [`docs/adr/`](docs/adr/):

| ADR | Decision |
|---|---|
| [0001](docs/adr/0001-language-rust.md) | Implementation language: Rust |
| [0002](docs/adr/0002-query-engine-datafusion.md) | Query engine: Apache DataFusion |
| [0003](docs/adr/0003-storage-parquet-iceberg.md) | Storage: Apache Parquet + Iceberg |
| [0004](docs/adr/0004-catalog-apache-polaris.md) | Catalog: Apache Polaris |
| [0005](docs/adr/0005-wire-protocol-dual.md) | Wire protocol: PostgreSQL + Snowflake REST API v2 |
| [0006](docs/adr/0006-license-apache2.md) | License: Apache 2.0 |
| [0007](docs/adr/0007-monorepo.md) | Repository structure: monorepo |

---

## Repository layout

```
opensnow/
├── crates/
│   ├── opensnow-common/     # Shared types, errors, config
│   ├── opensnow-catalog/    # Iceberg REST catalog client (Apache Polaris)
│   ├── opensnow-storage/    # Parquet + S3 I/O, Iceberg table format
│   ├── opensnow-sql/        # Snowflake SQL dialect extensions (DataFusion)
│   ├── opensnow-compute/    # Virtual warehouse manager + query execution
│   ├── opensnow-auth/       # JWT auth, RBAC, session management
│   ├── opensnow-server/     # Wire protocol servers (PG + REST API v2)
│   └── opensnow-cli/        # SnowSQL-compatible CLI
├── deploy/
│   ├── helm/                # Helm chart
│   └── terraform/aws/       # Terraform AWS module (EKS + S3 + IAM)
├── docs/
│   ├── architecture/        # Architecture overview
│   └── adr/                 # Architecture Decision Records
├── site/                    # Documentation site (Astro)
├── tests/
│   ├── integration/         # End-to-end SQL correctness tests
│   └── tpch/                # TPC-H performance benchmark suite
├── docker-compose.yml       # Local dev stack
├── Dockerfile.server        # Cloud Services node container image
└── Dockerfile.worker        # Warehouse Worker container image
```

---

## Contributing

Contributions are welcome. Please read `CONTRIBUTING.md` before submitting a
pull request. (CONTRIBUTING.md is coming soon.)

All contributions are made under the Apache 2.0 license.

---

## License

Apache License 2.0 — see [LICENSE](LICENSE) for the full text.
