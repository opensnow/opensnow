# TPC-H Benchmark Suite

Performance benchmarks based on the [TPC-H](https://www.tpc.org/tpch/)
industry-standard analytical query benchmark. Used to track query performance
across releases and compare OpenSnow against Snowflake on equivalent
hardware.

## About TPC-H

TPC-H simulates a decision support system with 8 tables and 22 analytical
queries covering a range of complexity: simple scans, multi-table joins,
aggregations, subqueries, and sorting. It is widely used to benchmark
analytical query engines.

## Scale Factors

| Scale Factor | Data size (approx.) | Use |
|---|---|---|
| SF1 | ~1 GB | Local dev / CI smoke test |
| SF10 | ~10 GB | Pre-release regression check |
| SF100 | ~100 GB | Release benchmarking on AWS |
| SF1000 | ~1 TB | Large-scale performance validation |

## Structure

```
tpch/
├── data/               ← generated TPC-H data files (git-ignored)
├── schema/
│   └── create.sql      ← TPC-H table DDL (Snowflake SQL dialect)
├── queries/
│   ├── q01.sql         ← Query 1: Pricing Summary Report
│   ├── q02.sql         ← Query 2: Minimum Cost Supplier
│   │   ...
│   └── q22.sql         ← Query 22: Global Sales Opportunity
├── results/            ← benchmark run results (git-ignored)
│   └── .gitkeep
└── run.sh              ← benchmark runner script (TODO: implement)
```

## Running

```bash
# Generate TPC-H data at scale factor 1 (requires tpch-dbgen)
cd tests/tpch
./run.sh generate --scale-factor 1

# Load data into OpenSnow
./run.sh load --scale-factor 1

# Run all 22 queries and record timing
./run.sh bench --scale-factor 1 --runs 3

# Results written to results/sf1-<timestamp>.json
```

## TODO

- [ ] Add TPC-H schema DDL in Snowflake SQL dialect (`schema/create.sql`)
- [ ] Add all 22 TPC-H query files (`queries/q01.sql` … `q22.sql`)
- [ ] Implement `run.sh` benchmark runner
- [ ] Add CI job that runs SF1 as a regression check on main branch pushes
- [ ] Publish SF100 results in `docs/benchmarks/` for each release
