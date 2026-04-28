#!/usr/bin/env python3
"""Parse classeq2 JSONL telemetry logs and export metrics as CSV/Parquet.

Usage:
    python collect_metrics.py run.jsonl -o metrics.csv
    python collect_metrics.py run.jsonl --format parquet -o metrics.parquet
    python collect_metrics.py run.jsonl --summary
"""

import argparse
import json
import sys
from pathlib import Path


def parse_jsonl(path: Path) -> list[dict]:
    records = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                continue
    return records


def extract_placement_summary(records: list[dict]) -> list[dict]:
    """Extract CLIPLACE0002 summary records (one per run)."""
    rows = []
    for r in records:
        fields = r.get("fields", {})
        if fields.get("code") == "CLIPLACE0002":
            rows.append(
                {
                    "timestamp": r.get("timestamp"),
                    "total_seconds": fields.get("totalSeconds"),
                    "avg_seconds": fields.get("averageSeconds"),
                    "max_seconds": fields.get("maxSeconds"),
                    "sequences_placed": fields.get("sequencesPlaced"),
                }
            )
    return rows


def extract_per_sequence(records: list[dict]) -> list[dict]:
    """Extract per-sequence placement spans (UCPLACE0001/0002 pair)."""
    rows = []
    for r in records:
        span = r.get("span", {})
        fields = r.get("fields", {})
        # PlaceSingleSequence spans carry kmer diagnostics
        if span.get("name") == "PlaceSingleSequence":
            rows.append(
                {
                    "timestamp": r.get("timestamp"),
                    "query": fields.get("query"),
                    "query_id": fields.get("query_id"),
                    "kmers_count": fields.get("query.kmers.count"),
                    "kmers_tree_matches": fields.get(
                        "query.kmers.treeMatches"
                    ),
                    "kmers_build_time": fields.get("query.kmers.buildTime"),
                    "subject_query_matches": fields.get(
                        "subject.kmers.queryMatches"
                    ),
                    "subject_build_time": fields.get(
                        "subject.kmers.buildTime"
                    ),
                }
            )
    return rows


def extract_placement_outcomes(records: list[dict]) -> dict:
    """Count placement outcome codes from UCPLACE* events."""
    from collections import Counter

    codes = Counter()
    for r in records:
        code = r.get("fields", {}).get("code", "")
        if code.startswith("UCPLACE"):
            codes[code] += 1
    return dict(codes)


def print_summary(records: list[dict]) -> None:
    summaries = extract_placement_summary(records)
    outcomes = extract_placement_outcomes(records)
    per_seq = extract_per_sequence(records)

    print("=== Run Summary ===")
    for row in summaries:
        print(f"  Timestamp       : {row['timestamp']}")
        print(f"  Total time (s)  : {row['total_seconds']}")
        print(f"  Avg time (s)    : {row['avg_seconds']}")
        print(f"  Max time (s)    : {row['max_seconds']}")
        print(f"  Sequences placed: {row['sequences_placed']}")
        print()

    print("=== Placement Outcome Codes ===")
    for code, count in sorted(outcomes.items()):
        print(f"  {code}: {count}")

    if per_seq:
        kmers_counts = [
            r["kmers_count"] for r in per_seq if r["kmers_count"] is not None
        ]
        if kmers_counts:
            print(
                f"\n=== K-mer Stats (n={len(kmers_counts)} sequences) ==="
            )
            print(f"  Avg k-mers/query: {sum(kmers_counts)/len(kmers_counts):.1f}")
            print(f"  Min k-mers/query: {min(kmers_counts)}")
            print(f"  Max k-mers/query: {max(kmers_counts)}")


def to_csv(rows: list[dict], out_path: Path) -> None:
    import csv

    if not rows:
        print("No records to write.", file=sys.stderr)
        return
    with open(out_path, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)
    print(f"Written {len(rows)} rows to {out_path}")


def to_parquet(rows: list[dict], out_path: Path) -> None:
    try:
        import pandas as pd
    except ImportError:
        print(
            "pandas is required for parquet export: pip install pandas pyarrow",
            file=sys.stderr,
        )
        sys.exit(1)
    df = pd.DataFrame(rows)
    df.to_parquet(out_path, index=False)
    print(f"Written {len(rows)} rows to {out_path}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log_file", type=Path, help="JSONL log file from cls")
    parser.add_argument(
        "-o", "--output", type=Path, default=None, help="Output file path"
    )
    parser.add_argument(
        "--format",
        choices=["csv", "parquet"],
        default="csv",
        help="Output format (default: csv)",
    )
    parser.add_argument(
        "--mode",
        choices=["summary", "per-sequence", "outcomes"],
        default="summary",
        help="Which data to extract (default: summary)",
    )
    parser.add_argument(
        "--summary",
        action="store_true",
        help="Print human-readable summary to stdout and exit",
    )
    args = parser.parse_args()

    records = parse_jsonl(args.log_file)
    print(f"Parsed {len(records)} log records from {args.log_file}")

    if args.summary:
        print_summary(records)
        return

    if args.mode == "summary":
        rows = extract_placement_summary(records)
    elif args.mode == "per-sequence":
        rows = extract_per_sequence(records)
    else:
        rows = [
            {"code": k, "count": v}
            for k, v in extract_placement_outcomes(records).items()
        ]

    if args.output is None:
        stem = args.log_file.stem
        args.output = Path(f"{stem}_{args.mode}.{args.format}")

    if args.format == "csv":
        to_csv(rows, args.output)
    else:
        to_parquet(rows, args.output)


if __name__ == "__main__":
    main()
