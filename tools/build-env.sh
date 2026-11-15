#!/bin/sh
# Resolve or explicitly prepare a locked build environment. Never registers binfmt.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec python3 -I "$root/tools/build_env.py" "$@"
