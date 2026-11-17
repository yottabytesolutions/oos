#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# perf-guard.sh - the fork's performance guards, the `perf` step of the full gate (docs/development.md).
#
#   tools/perf-guard.sh [--build-only]
#
# 1. Deterministic guards (they also run in the normal `cargo test`): allocation counts of the batch read and decode,
#    the early trigger end on the generic batch path, and the equivalence tests of the fork's fast paths.
# 2. Timing guards: `#[ignore]` tests that compare a fork path with a reference copy of the path it replaced, in one
#    release run on one thread. They assert ratios, not times. In oos-ui these are the ignored tests whose name
#    contains `perf_guard`. Both best and p99 ratios are checked against the architecture host table.
# Host only: touches no hardware. Exits non-zero on the first failing guard.
set -eu
export OOS_PERF_TESTS=1
root=$(cd "$(dirname "$0")/.." && pwd)
. "$root/tools/output.sh"
export PATH="$HOME/.cargo/bin:$PATH"

case "${1-}" in
    ''|--build-only) ;;
    *) echo "usage: tools/perf-guard.sh [--build-only]" >&2; exit 2 ;;
esac
# Compilation can overlap other gate work; the timing guards still run alone. These profiles do not share build
# artifacts with the debug and test-release steps, so a separate Cargo lock avoids serializing their builds.
output=$CARGO_TARGET_DIR
CARGO_TARGET_DIR="$output/perf-build"
. "$root/tools/output.sh"
# Preserve Cargo's real status without a pipeline; keep complete observations for the budget reader.
mkdir -p "$CARGO_TARGET_DIR"
log="$output/perf-guard.log"
if [ "${1-}" = --build-only ]; then log="$output/perf-build.log"; fi
step_log="$output/perf-guard-step.log"
: > "$log"
started=$(date +%s)
run() {
    if "$@" > "$step_log" 2>&1; then
        cat "$step_log"
        cat "$step_log" >> "$log"
    else
        code=$?
        cat "$step_log"
        cat "$step_log" >> "$log"
        exit "$code"
    fi
}
cd "$root"

if [ "${1-}" = --build-only ]; then
    run cargo test --locked --profile perf --quiet --no-run -p oos-hal \
        --lib --test alloc_batch --test early_trigger_end --test perf_ratio
    run cargo test --locked --release --quiet --no-run -p oos-ui --test soak --bin oos-ui
    echo "perf guards: built"
    exit 0
fi

echo "== hal: deterministic guards"
run cargo test --locked --profile perf --quiet -p oos-hal --test alloc_batch --test early_trigger_end
run cargo test --locked --profile perf --quiet -p oos-hal --lib -- \
    fused_decode_equals_deinterleave_and_volts decode_into_kept_vectors_equals_decode unpad_in_place_equals_unpad \
    prearm_writes_the_same_sequence_as_the_plain_batch_loop dma_buf_is_reused_aligned_and_exact_with_short_reads

echo "== hal: timing guards (best and p99 ratios)"
run cargo test --locked --profile perf --quiet -p oos-hal --test perf_ratio -- --ignored --nocapture --test-threads=1

echo "== ui: zero-allocation hot paths"
run cargo test --locked --release --quiet -p oos-ui --test soak -- hot_paths_allocate_nothing_after_warmup --nocapture

echo "== ui: timing guards (best and p99 ratios)"
run cargo test --locked --release --quiet -p oos-ui --bin oos-ui -- --ignored --nocapture --test-threads=1 perf_guard

python3 "$root/tools/perf-budgets.py" --log "$log"
echo "perf guards: all passed ($(( $(date +%s) - started )) s)"
