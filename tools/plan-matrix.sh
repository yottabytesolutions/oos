#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# plan-matrix.sh - register sequences of a fixed set of acquisitions, for comparing two builds of oos-fpgad.
#
#   tools/plan-matrix.sh [path/to/oos-fpgad] > plan-OLD.txt
#
# `oos-fpgad plan` only prints, it touches no hardware, so this is safe on the unit (busybox sh) and on the Mac. Run it
# with the old build still on the unit BEFORE installing a new image, and with the new build, then `diff`: the analog
# path must not change at 1.25 GHz. Needs the calibration files: on the unit
# /rigol/data, on a host computer `OOS_CAL_DIR=local/cal`.
f=${1:-oos-fpgad}
run() {
    echo "### $*"
    "$f" plan "$@" 2>&1
}
# One whitespace-separated argv per line, shared with the host gate. Disable globbing before splitting.
set -f
while IFS= read -r args; do
    # shellcheck disable=SC2086 # Word splitting turns the shared case into arguments.
    run $args
done < "$(dirname "$0")/plan-matrix-cases.txt"
