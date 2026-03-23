# Integration Tests

End-to-end tests that run SQL against a live OpenSnow instance and verify
correctness of the full stack: wire protocol → query dispatch → compute →
storage → result serialisation.

## Strategy

Integration tests are written as SQL scripts (`.sql` files) paired with
expected output files (`.expected` files). The test harness:

1. Starts a local OpenSnow stack via Docker Compose (`docker compose up -d`)
2. Connects over the PostgreSQL wire protocol using `tokio-postgres`
3. Executes each SQL script
4. Diffs the output against the `.expected` file
5. Tears down the stack

## Categories

| Directory | What it tests |
|---|---|
| `ddl/` | CREATE / DROP / ALTER / UNDROP for databases, schemas, tables |
| `dml/` | INSERT, UPDATE, DELETE, COPY INTO |
| `select/` | SELECT, JOINs, aggregates, window functions, CTEs |
| `snowflake-dialect/` | QUALIFY, VARIANT, AT/BEFORE, CLONE — Snowflake extensions |
| `warehouses/` | CREATE WAREHOUSE, SUSPEND/RESUME, multi-warehouse isolation |
| `time-travel/` | AT TIMESTAMP, AT OFFSET, AT STATEMENT, UNDROP |
| `cloning/` | CREATE TABLE/SCHEMA/DATABASE CLONE |
| `auth/` | RBAC — roles, grants, privilege enforcement |
| `connectors/` | snowflake-connector-python, dbt-snowflake, JDBC compatibility |

## Running

```bash
# Start the local stack
docker compose up -d

# Run all integration tests
cargo test --test integration

# Run a specific category
cargo test --test integration ddl

# Tear down
docker compose down -v
```

## TODO

- [ ] Implement the test harness runner
- [ ] Add DDL smoke tests (Phase 1)
- [ ] Add Snowflake dialect extension tests (Phase 2)
- [ ] Add time travel and clone tests (Phase 3)
- [ ] Add connector compatibility tests (Phase 2)
