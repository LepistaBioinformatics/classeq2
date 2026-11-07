"""Summaries for bench.sh (005-placement-performance).

summarize.py --log LOG.jsonl   print the average and maximum seconds per
                               query from the CLIPLACE0002 record
summarize.py RESULTS.tsv       print min, median and max wall time, the peak
                               RSS and the per-query times per configuration
"""
import csv
import json
import statistics
import sys

if sys.argv[1] == "--log":
    with open(sys.argv[2]) as fh:
        for line in fh:
            record = json.loads(line)
            fields = record.get("fields", record)
            if "averageSeconds" in fields:
                print(fields["averageSeconds"], fields["maxSeconds"])
                break
        else:
            print("nan nan")
    sys.exit()

groups = {}
with open(sys.argv[1]) as fh:
    for row in csv.DictReader(fh, delimiter="\t"):
        key = (row["label"], row["db"], row["queries"], row["threads"])
        groups.setdefault(key, []).append(row)

print("| label | db | queries | -t | runs | wall min | wall median | wall max "
      "| peak RSS MB | avg query ms | max query ms |")
print("|---|---|---|---|---|---|---|---|---|---|---|")
for (label, db, queries, threads), rows in groups.items():
    walls = [float(r["wall_s"]) for r in rows]
    rss = max(int(r["max_rss_kb"]) for r in rows) / 1024
    avg = statistics.median(float(r["avg_query_s"]) for r in rows) * 1000
    worst = max(float(r["max_query_s"]) for r in rows) * 1000
    print(f"| {label} | {db} | {queries} | {threads} | {len(rows)} "
          f"| {min(walls):.2f} | {statistics.median(walls):.2f} "
          f"| {max(walls):.2f} | {rss:.0f} | {avg:.2f} | {worst:.1f} |")
