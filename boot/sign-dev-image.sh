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
[ -f "$here/out/oos-dev.build" ] || { echo "run boot/build-dev-image.sh first" >&2; exit 1; }
exec cargo run --offline --release --manifest-path "$root/fpgad/Cargo.toml" --bin oos-update -- pack \
	--image "$here/out/oos-dev.img" --build "$(cat "$here/out/oos-dev.build")" \
	--out "$here/out/oos-dev.oosupd" "$@"
