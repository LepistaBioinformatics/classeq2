#!/usr/bin/env bash
# MEAS-01 equivalence check (005-placement-performance, design D7).
#
# Usage:
#   equivalence.sh BASE_CLS CORPUS_DIR QUERIES [CAND_CLS]
#
# CORPUS_DIR holds ref.nwk and ref.fasta. The legacy database db.cls is built
# there with BASE_CLS when it is missing. Without CAND_CLS only run A (the
# reference outputs) is produced, and it is reused on later calls.
#
# Runs, each in YAML and JSONL and with and without --remove-intersection:
#   A  BASE_CLS on db.cls (legacy)
#   B  CAND_CLS on db.cls (legacy load)
#   C  CAND_CLS on db2.cls (v2 written by CAND_CLS build-db)
#   D  CAND_CLS on db3.cls (v2 written by CAND_CLS convert -f binary)
# Pass: normalized A == B == C == D, sorted .error files equal, and
# db2.cls and db3.cls byte-identical.
set -euo pipefail

BASE=$1
CORPUS=$(cd "$2" && pwd)
QUERIES=$(cd "$(dirname "$3")" && pwd)/$(basename "$3")
CAND=${4:-}
THREADS=${THREADS:-8}
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=$CORPUS/equiv/$(basename "$QUERIES" .fasta)
mkdir -p "$OUT"

if [ ! -f "$CORPUS/db.cls" ]; then
    "$BASE" build-db "$CORPUS/ref.nwk" "$CORPUS/ref.fasta" -o "$CORPUS/db.cls"
fi

place() { # bin db run variant format
    local bin=$1 db=$2 run=$3 variant=$4 fmt=$5 extra=()
    [ "$variant" = ri ] && extra=(-r)
    rm -f "$OUT/$run-$variant.$fmt" "$OUT/$run-$variant.error"
    "$bin" -t "$THREADS" --log-level error place -d "$db" \
        -o "$OUT/$run-$variant" --out-format "$fmt" -f "${extra[@]}" \
        "$QUERIES" >/dev/null 2>&1
    python3 -I "$HERE/normalize.py" "$OUT/$run-$variant.$fmt" \
        >"$OUT/$run-$variant.$fmt.norm"
    if [ -f "$OUT/$run-$variant.error" ]; then
        sort "$OUT/$run-$variant.error" >"$OUT/$run-$variant.$fmt.error.sorted"
    else
        : >"$OUT/$run-$variant.$fmt.error.sorted"
    fi
}

VARIANTS=(default ri)
FORMATS=(yaml jsonl)

for v in "${VARIANTS[@]}"; do
    for f in "${FORMATS[@]}"; do
        if [ ! -f "$OUT/A-$v.$f.norm" ]; then
            place "$BASE" "$CORPUS/db.cls" A "$v" "$f"
        fi
    done
done

[ -z "$CAND" ] && { echo "reference ready: $OUT"; exit 0; }

"$CAND" build-db "$CORPUS/ref.nwk" "$CORPUS/ref.fasta" -o "$OUT/db2.cls" >/dev/null 2>&1
"$CAND" convert database -d "$CORPUS/db.cls" -f binary -o "$OUT/db3" >/dev/null 2>&1

status=0
if cmp -s "$OUT/db2.cls" "$OUT/db3.cls"; then
    echo "PASS cmp db2.cls db3.cls"
else
    echo "FAIL cmp db2.cls db3.cls"; status=1
fi

for run in B:db.cls C:db2.cls D:db3.cls; do
    name=${run%%:*} db=${run#*:}
    [ "$db" = db.cls ] && db=$CORPUS/db.cls || db=$OUT/$db
    for v in "${VARIANTS[@]}"; do
        for f in "${FORMATS[@]}"; do
            place "$CAND" "$db" "$name" "$v" "$f"
            if cmp -s "$OUT/A-$v.$f.norm" "$OUT/$name-$v.$f.norm" &&
                cmp -s "$OUT/A-$v.$f.error.sorted" "$OUT/$name-$v.$f.error.sorted"; then
                echo "PASS $name $v $f ($(wc -l <"$OUT/A-$v.$f.norm") records)"
            else
                echo "FAIL $name $v $f"; status=1
            fi
        done
    done
done
exit $status
