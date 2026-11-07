"""Print placement records in a canonical, order-free form.

Usage: normalize.py <placements.yaml|placements.jsonl>

Records are sorted by query header and printed one per line as JSON with
sorted keys, so two runs can be compared with diff even when `-t > 1`
writes them in a different order.
"""
import json
import sys

import yaml

path = sys.argv[1]
with open(path) as fh:
    if path.endswith(".jsonl"):
        records = [json.loads(line) for line in fh if line.strip()]
    else:
        loader = getattr(yaml, "CSafeLoader", yaml.SafeLoader)
        records = [doc for doc in yaml.load_all(fh, Loader=loader) if doc]

records.sort(key=lambda record: record["query"])
for record in records:
    print(json.dumps(record, sort_keys=True))
