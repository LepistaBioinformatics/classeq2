#!/usr/bin/env bash
# Placement benchmarks (005-placement-performance, MEAS-02 to MEAS-04,
# design D6). Run it in the profiling image (perf.Dockerfile) or on any host
# with GNU time, on a quiet machine with a warm page cache.
#
# Usage:
#   bench.sh LABEL CLS DB QUERIES THREADS RUNS OUT_TSV
#
# Appends one row per run to OUT_TSV:
#   label db queries threads run wall_s max_rss_kb avg_query_s max_query_s
# The per-query average and maximum come from the CLIPLACE0002 log record.
# Summarize with summarize.py.
set -euo pipefail

LABEL=$1 CLS=$2 DB=$3 QUERIES=$4 THREADS=$5 RUNS=$6 OUT=$7
HERE=$(cd "$(dirname "$0")" && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Warm the page cache, as every run after the first would.
cat "$DB" "$QUERIES" >/dev/null

[ -f "$OUT" ] || printf 'label\tdb\tqueries\tthreads\trun\twall_s\tmax_rss_kb\tavg_query_s\tmax_query_s\n' >"$OUT"

for run in $(seq 1 "$RUNS"); do
    rm -f "$TMP"/out.* "$TMP"/log.*
    /usr/bin/time -f '%e %M' -o "$TMP/time" \
        "$CLS" -t "$THREADS" --log-level info --log-format jsonl \
        --log-file "$TMP/log" place -d "$DB" -o "$TMP/out" -f "$QUERIES" \
        >/dev/null 2>&1
    read -r wall rss <"$TMP/time"
    read -r avg max < <(python3 -I "$HERE/summarize.py" --log "$TMP/log.jsonl")
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$LABEL" "$DB" \
        "$(basename "$QUERIES")" "$THREADS" "$run" "$wall" "$rss" "$avg" "$max" \
        | tee -a "$OUT"
done
