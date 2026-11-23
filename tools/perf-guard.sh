#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# perf-guard.sh - the fork's performance guards, the `perf` step of the full gate (docs/development.md).
#
#   tools/perf-guard.sh [--build-only]
#
# Run deterministic allocation, early-trigger, equality and bounded-work guards,
# plus interleaved CPU-cost ratios against the reference paths in the host budget table.
# Compare CPU in a serial release test process. Reported wall timings are diagnostics.
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
# HAL reuses test-release artifacts. UI uses the shipping release profile.
# Both use the same target directory as the other gate steps.
output=$CARGO_TARGET_DIR
# Preserve Cargo's real status without a pipeline; keep complete observations for the budget reader.
mkdir -p "$CARGO_TARGET_DIR"
log="$output/perf-guard.log"
if [ "${1-}" = --build-only ]; then log="$output/perf-build.log"; fi
step_log="$output/perf-guard-step.log"
: > "$log"
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
    run cargo test --locked --profile test-release --quiet --no-run -p oos-hal \
        --lib --test alloc_batch --test early_trigger_end --test perf_ratio
    run cargo test --locked --release --quiet --no-run -p oos-ui --test soak --bin oos-ui
    echo "perf guards: built"
    exit 0
fi

echo "== hal: deterministic guards"
run cargo test --locked --profile test-release --quiet -p oos-hal --test alloc_batch --test early_trigger_end -- --nocapture
run cargo test --locked --profile test-release --quiet -p oos-hal --lib -- \
    fused_decode_equals_deinterleave_and_volts decode_into_kept_vectors_equals_decode unpad_in_place_equals_unpad \
    prearm_writes_the_same_sequence_as_the_plain_batch_loop dma_buf_is_reused_aligned_and_exact_with_short_reads

echo "== hal: CPU guards (best and p99 ratios)"
run cargo test --locked --profile test-release --quiet -p oos-hal --test perf_ratio -- --ignored --nocapture --test-threads=1

echo "== ui: zero-allocation hot paths"
run cargo test --locked --release --quiet -p oos-ui --test soak -- hot_paths_allocate_nothing_after_warmup --nocapture

echo "== ui: work and CPU guards (M12 draw-route diagnostics)"
run cargo test --locked --release --quiet -p oos-ui --bin oos-ui -- --ignored --nocapture --test-threads=1 perf_guard

python3 "$root/tools/perf-budgets.py" --log "$log"
echo "perf guards: all passed"
