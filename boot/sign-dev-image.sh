#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
# Sign an already built image. Packaging as GEL is a separate step.
# Usage: boot/sign-dev-image.sh [--key PATH | --community]
# OOS_SIGNING_KEY selects a key when --key is absent.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
if [ "$#" = 1 ] && [ "$1" = --help ]; then
	echo "Usage: boot/sign-dev-image.sh [--key PATH | --community]"
	echo "Or set OOS_SIGNING_KEY. No signing key is discovered automatically."
	exit 0
fi
case "$#:${1:-}" in
	0:) ;;
	1:--community) ;;
	2:--key) [ -n "$2" ] || { echo "Usage: boot/sign-dev-image.sh [--key PATH | --community]" >&2; exit 1; } ;;
	*) echo "Usage: boot/sign-dev-image.sh [--key PATH | --community]" >&2; exit 1 ;;
esac
. "$root/tools/output.sh"
out="$CARGO_TARGET_DIR/image"
before=$(python3 "$root/tools/output.py" revision)
python3 "$root/tools/output.py" verify "$out/oos-dev.img"
[ -f "$out/oos-dev.build" ] || { echo "run boot/build-dev-image.sh first" >&2; exit 1; }
python3 "$root/tools/output.py" verify "$out/oos-dev.build"
cargo run --offline --locked --release --manifest-path "$root/fpgad/Cargo.toml" --bin oos-update -- pack \
	--image "$out/oos-dev.img" --build "$(cat "$out/oos-dev.build")" \
	--out "$out/oos-dev.oosupd" "$@"
python3 "$root/tools/output.py" verify "$out/oos-dev.img" "$out/oos-dev.build"
python3 "$root/tools/output.py" stamp "$out/oos-dev.oosupd" "$before"
