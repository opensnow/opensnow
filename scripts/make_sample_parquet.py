#!/usr/bin/env python3
"""Generate a small Parquet fixture for local OpenSnow testing."""

from pathlib import Path
import argparse


def main() -> None:
    parser = argparse.ArgumentParser(description="Create a sample parquet file")
    parser.add_argument(
        "--out",
        default="tests/fixtures/sample.parquet",
        help="Output parquet path (default: tests/fixtures/sample.parquet)",
    )
    args = parser.parse_args()

    try:
        import pyarrow as pa
        import pyarrow.parquet as pq
    except ModuleNotFoundError as exc:
        raise SystemExit(
            "pyarrow is required. Install with: python3 -m pip install --user pyarrow"
        ) from exc

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)

    table = pa.table(
        {
            "id": [1, 2, 3],
            "name": ["alice", "bob", "carol"],
            "score": [98.5, 87.0, 91.25],
        }
    )
    pq.write_table(table, out)
    print(out.resolve())


if __name__ == "__main__":
    main()
