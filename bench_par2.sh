#!/usr/bin/env bash
# PAR2 creation benchmark: pesto vs parpar vs par2
# Tests all three tools on file1G.bin, file5G.bin, file10G.bin
# Recovery: 10% (pesto default)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PESTO="$SCRIPT_DIR/target/release/pesto"
OUTDIR="$SCRIPT_DIR/bench_par2_out"
FILES=("file1G.bin" "file5G.bin" "file10G.bin")

# PAR2 params equivalent to pesto defaults: 10% recovery, 2000 slices
PAR2_REDUNDANCY=10
PARPAR_RECOVERY="10%"
PAR2_RECOVERY="10"

# ── helpers ──────────────────────────────────────────────────────────────────

color_bold() { printf '\033[1m%s\033[0m' "$1"; }
color_green() { printf '\033[32m%s\033[0m' "$1"; }
color_yellow() { printf '\033[33m%s\033[0m' "$1"; }
color_cyan() { printf '\033[36m%s\033[0m' "$1"; }

hr() { printf '%0.s─' {1..70}; echo; }

elapsed_to_str() {
    local t=$1
    printf '%dm%02ds' "$((t/60))" "$((t%60))"
}

# Returns throughput in MB/s given file size bytes and elapsed seconds
throughput() {
    local bytes=$1
    local elapsed_ms=$2
    awk "BEGIN { printf \"%.1f\", ($bytes / 1048576) / ($elapsed_ms / 1000) }"
}

file_size_bytes() {
    stat -c%s "$1"
}

# ── setup ────────────────────────────────────────────────────────────────────

mkdir -p "$OUTDIR"

echo
color_bold "PAR2 Creation Benchmark"; echo
color_bold "Tools: pesto | parpar | par2"; echo
echo "Recovery: ${PAR2_REDUNDANCY}%  |  Files: ${FILES[*]}"
hr

declare -A RESULTS  # key: "tool:file" → "elapsed_ms:bytes"

run_bench() {
    local tool=$1      # pesto | parpar | par2
    local file=$2      # relative filename
    local src="$SCRIPT_DIR/$file"
    local workdir="$OUTDIR/${tool}_${file}"
    local bytes
    bytes=$(file_size_bytes "$src")

    mkdir -p "$workdir"

    # Drop page cache so disk reads are cold & fair
    sync
    echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null 2>&1 || true

    local start_ms end_ms elapsed_ms
    start_ms=$(date +%s%3N)

    case "$tool" in
        pesto)
            "$PESTO" --par2-only --par2 "$PAR2_REDUNDANCY" "$src" \
                > "$workdir/stdout.log" 2>&1
            ;;
        parpar)
            parpar \
                -s2000 -S \
                -r "${PARPAR_RECOVERY}" \
                -o "$workdir/${file}.par2" \
                "$src" \
                > "$workdir/stdout.log" 2>&1
            ;;
        par2)
            par2 create \
                -r"${PAR2_RECOVERY}" \
                -n1 \
                "$workdir/${file}.par2" \
                "$src" \
                > "$workdir/stdout.log" 2>&1
            ;;
    esac

    end_ms=$(date +%s%3N)
    elapsed_ms=$(( end_ms - start_ms ))
    RESULTS["${tool}:${file}"]="${elapsed_ms}:${bytes}"
}

# ── run all benchmarks ────────────────────────────────────────────────────────

TOOLS=("pesto" "parpar" "par2")

for file in "${FILES[@]}"; do
    color_bold "  File: $file ($(numfmt --to=iec-i --suffix=B "$(file_size_bytes "$SCRIPT_DIR/$file")"))"
    echo
    for tool in "${TOOLS[@]}"; do
        printf "    %-8s  running..." "$tool"
        run_bench "$tool" "$file"
        local_ms="${RESULTS["${tool}:${file}"]%%:*}"
        local_bytes="${RESULTS["${tool}:${file}"]##*:}"
        tp=$(throughput "$local_bytes" "$local_ms")
        printf "\r    %-8s  %s  →  %s MB/s\n" \
            "$tool" \
            "$(elapsed_to_str $(( local_ms / 1000 )))" \
            "$tp"
    done
    echo
done

# ── ranking ───────────────────────────────────────────────────────────────────

hr
color_bold "RANKING — by throughput (higher = faster)"; echo
hr

for file in "${FILES[@]}"; do
    size_bytes=$(file_size_bytes "$SCRIPT_DIR/$file")
    size_hr=$(numfmt --to=iec-i --suffix=B "$size_bytes")

    color_cyan "  $file ($size_hr)"; echo

    # Build sortable list: throughput_x10 tool elapsed_ms
    declare -a entries=()
    for tool in "${TOOLS[@]}"; do
        key="${tool}:${file}"
        elapsed_ms="${RESULTS[$key]%%:*}"
        bytes="${RESULTS[$key]##*:}"
        tp_int=$(awk "BEGIN { printf \"%d\", ($bytes / 1048576) / ($elapsed_ms / 1000) * 10 }")
        entries+=("$tp_int $tool $elapsed_ms $bytes")
    done

    # Sort descending by throughput
    IFS=$'\n' sorted=($(printf '%s\n' "${entries[@]}" | sort -rn))
    unset IFS

    rank=1
    for entry in "${sorted[@]}"; do
        read -r tp_int tool elapsed_ms bytes <<< "$entry"
        tp=$(throughput "$bytes" "$elapsed_ms")
        elapsed_s=$(awk "BEGIN { printf \"%.1f\", $elapsed_ms / 1000 }")

        medal=""
        case $rank in
            1) medal="🥇";;
            2) medal="🥈";;
            3) medal="🥉";;
        esac

        printf "    %s  #%d  %-8s  %6s MB/s  %6.1fs\n" \
            "$medal" "$rank" "$tool" "$tp" "$elapsed_s"
        (( rank++ ))
    done
    echo
done

hr
color_bold "Output files saved in: $OUTDIR"; echo
echo
