#!/usr/bin/env bash
# run_benchmarks.sh — Comparative performance benchmarks for classeq2
#
# Runs classeq2 (and optionally EPA-ng / RAPPAS2) over a matrix of thread
# counts and query set sizes, collecting time and memory for each combination.
#
# Prerequisites:
#   cargo install (builds cls in release mode)
#   /usr/bin/time -v  (GNU time, for peak RSS — install via `time` package)
#   Optional: epa-ng, rappas2 in PATH for cross-tool comparisons
#
# Usage:
#   bash scripts/run_benchmarks.sh --tree ref.nwk --msa ref.fasta --queries queries.fasta
#   bash scripts/run_benchmarks.sh --tree ref.nwk --msa ref.fasta --queries queries.fasta \
#       --tools "classeq2 epa-ng" --threads "1 2 4 8 16"

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────
TREE_FILE=""
MSA_FILE=""
QUERY_FILE=""
OUTDIR="benchmark_results/$(date +%Y%m%d_%H%M%S)"
TOOLS="classeq2"
THREAD_COUNTS="1 2 4 8"
REPLICATES=3
K_SIZE=35
M_SIZE=4
MIN_BRANCH_SUPPORT=70
CLS_BIN="./target/release/cls"
LOG_FORMAT="jsonl"

# ── Argument parsing ──────────────────────────────────────────────────────────
usage() {
    echo "Usage: $0 --tree FILE --msa FILE --queries FILE [options]"
    echo ""
    echo "Required:"
    echo "  --tree FILE       Reference tree in Newick format"
    echo "  --msa FILE        Multiple sequence alignment (FASTA)"
    echo "  --queries FILE    Query sequences (FASTA)"
    echo ""
    echo "Optional:"
    echo "  --outdir DIR      Output directory (default: benchmark_results/<timestamp>)"
    echo "  --tools LIST      Space-separated tools to benchmark (default: 'classeq2')"
    echo "                    Options: classeq2, epa-ng, rappas2"
    echo "  --threads LIST    Space-separated thread counts (default: '1 2 4 8')"
    echo "  --replicates N    Number of replicates per condition (default: 3)"
    echo "  --k-size N        Kmer size for classeq2 (default: 35)"
    echo "  --m-size N        Minimizer size for classeq2 (default: 4)"
    echo "  --cls-bin PATH    Path to cls binary (default: ./target/release/cls)"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case $1 in
        --tree)       TREE_FILE="$2";        shift 2 ;;
        --msa)        MSA_FILE="$2";         shift 2 ;;
        --queries)    QUERY_FILE="$2";       shift 2 ;;
        --outdir)     OUTDIR="$2";           shift 2 ;;
        --tools)      TOOLS="$2";            shift 2 ;;
        --threads)    THREAD_COUNTS="$2";    shift 2 ;;
        --replicates) REPLICATES="$2";       shift 2 ;;
        --k-size)     K_SIZE="$2";           shift 2 ;;
        --m-size)     M_SIZE="$2";           shift 2 ;;
        --cls-bin)    CLS_BIN="$2";          shift 2 ;;
        -h|--help)    usage ;;
        *) echo "Unknown option: $1"; usage ;;
    esac
done

[[ -z "$TREE_FILE" || -z "$MSA_FILE" || -z "$QUERY_FILE" ]] && {
    echo "Error: --tree, --msa, and --queries are required."
    usage
}

mkdir -p "$OUTDIR"
SUMMARY_CSV="$OUTDIR/summary.csv"
echo "tool,threads,replicate,wall_seconds,peak_rss_kb,exit_code" > "$SUMMARY_CSV"

log() { echo "[$(date +%H:%M:%S)] $*"; }

# ── Time wrapper ──────────────────────────────────────────────────────────────
# Returns "wall_s peak_rss_kb" via a tmp file
time_cmd() {
    local timefile
    timefile=$(mktemp)
    /usr/bin/time -v -o "$timefile" "$@" 2>/dev/null || true
    local wall
    wall=$(grep "Elapsed (wall clock)" "$timefile" | awk '{print $NF}' | \
           awk -F: '{ if (NF==2) print $1*60+$2; else print $1*3600+$2*60+$3 }')
    local rss
    rss=$(grep "Maximum resident" "$timefile" | awk '{print $NF}')
    rm -f "$timefile"
    echo "${wall:-0} ${rss:-0}"
}

# ── Build classeq2 database ───────────────────────────────────────────────────
DB_FILE="$OUTDIR/database.cls"

if [[ "$TOOLS" == *"classeq2"* ]]; then
    if [[ ! -f "$CLS_BIN" ]]; then
        log "Building classeq2 in release mode..."
        cargo build --release --quiet
    fi

    log "Building classeq2 database..."
    "$CLS_BIN" build-db \
        "$TREE_FILE" "$MSA_FILE" \
        -o "$DB_FILE" \
        -k "$K_SIZE" \
        -m "$M_SIZE" \
        -s "$MIN_BRANCH_SUPPORT" 2>/dev/null

    log "Database built: $DB_FILE"
fi

# ── Build RAPPAS2 database ────────────────────────────────────────────────────
RAPPAS_DB="$OUTDIR/rappas_db"
if [[ "$TOOLS" == *"rappas2"* ]] && command -v rappas2 &>/dev/null; then
    log "Building RAPPAS2 database..."
    rappas2 build -t "$TREE_FILE" -a "$MSA_FILE" -k 9 -o "$RAPPAS_DB" 2>/dev/null
fi

# ── Main benchmark loop ───────────────────────────────────────────────────────
for tool in $TOOLS; do
    for threads in $THREAD_COUNTS; do
        for rep in $(seq 1 "$REPLICATES"); do
            log "  $tool | threads=$threads | rep=$rep"

            out="$OUTDIR/${tool}_t${threads}_r${rep}"
            log_file="${out}.log.${LOG_FORMAT}"
            exit_code=0

            case "$tool" in
                classeq2)
                    read -r wall rss < <(time_cmd \
                        env RAYON_NUM_THREADS="$threads" \
                        "$CLS_BIN" \
                        --log-level info \
                        --log-format "$LOG_FORMAT" \
                        --log-file "$log_file" \
                        place \
                        -d "$DB_FILE" \
                        "$QUERY_FILE" \
                        -o "${out}.results" \
                        --overwrite
                    ) || exit_code=$?
                    ;;

                epa-ng)
                    if ! command -v epa-ng &>/dev/null; then
                        log "  epa-ng not found, skipping"
                        continue
                    fi
                    read -r wall rss < <(time_cmd \
                        epa-ng \
                        --redo \
                        --threads "$threads" \
                        --tree "$TREE_FILE" \
                        --ref-msa "$MSA_FILE" \
                        --query "$QUERY_FILE" \
                        --outdir "${out}_epa/"
                    ) || exit_code=$?
                    ;;

                rappas2)
                    if ! command -v rappas2 &>/dev/null; then
                        log "  rappas2 not found, skipping"
                        continue
                    fi
                    read -r wall rss < <(time_cmd \
                        rappas2 place \
                        -d "$RAPPAS_DB/" \
                        -q "$QUERY_FILE" \
                        -o "${out}_rappas.csv"
                    ) || exit_code=$?
                    ;;

                *)
                    log "  Unknown tool '$tool', skipping"
                    continue
                    ;;
            esac

            echo "$tool,$threads,$rep,$wall,$rss,$exit_code" >> "$SUMMARY_CSV"
        done
    done
done

log "Benchmarks complete. Results in: $OUTDIR"
log "Summary CSV: $SUMMARY_CSV"

# ── Quick stats ───────────────────────────────────────────────────────────────
echo ""
echo "=== Mean wall time (seconds) by tool × threads ==="
awk -F, 'NR>1 { sum[$1","$2]+=$4; cnt[$1","$2]++ }
         END { for (k in sum) printf "  %-20s %6.2f s\n", k, sum[k]/cnt[k] }' \
    "$SUMMARY_CSV" | sort
