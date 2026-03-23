# Changelog

All notable changes to OpenSnow are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
OpenSnow uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- Initial project scaffolding: Rust workspace with 8 crates
  (`opensnow-common`, `opensnow-catalog`, `opensnow-storage`, `opensnow-sql`,
  `opensnow-compute`, `opensnow-auth`, `opensnow-server`, `opensnow-cli`)
- Architecture documentation (`docs/architecture/overview.md`) covering the
  three-layer design, query lifecycle, and deployment model
- Architecture Decision Records for all major technology choices:
  Rust, Apache DataFusion, Parquet + Iceberg, Apache Polaris, dual wire
  protocol (PostgreSQL + Snowflake REST API v2), Apache 2.0 license, monorepo
- Docker Compose local development stack (MinIO, PostgreSQL, Apache Polaris,
  opensnow-server stub, opensnow-worker stub)
- Multi-stage Dockerfiles for `opensnow-server` and `opensnow-worker`
- Helm chart for Kubernetes deployment (Cloud Services, Warehouse Worker,
  Polaris, PostgreSQL)
- Terraform AWS module scaffolding (EKS, S3, IAM/IRSA)
- GitHub Actions CI workflow (Rust test/lint, Helm lint, Terraform validate)
- GitHub Actions release workflow (container images, Helm chart, CLI binaries)
- Integration test and TPC-H benchmark suite placeholders

---

<!-- Releases are added below this line, newest first -->

<!--
## [0.1.0] - YYYY-MM-DD

### Added
### Changed
### Fixed
### Removed
-->

[Unreleased]: https://github.com/opensnow/opensnow/compare/HEAD...HEAD
