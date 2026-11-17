# SPDX-License-Identifier: GPL-2.0-or-later
# Source after setting root to the worktree root. Relative overrides are root-relative.
CARGO_TARGET_DIR=$(python3 "${root:?}/tools/output.py" target) || exit 1
export CARGO_TARGET_DIR
