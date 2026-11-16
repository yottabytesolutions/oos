#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build.sh: oos-ui for the scope (static aarch64 musl) and for the host (off-screen test runner).
#   ./build.sh            both
#   ./build.sh test       render tests/ to unique test-shots.* directories under the selected target (needs Pillow)
#   OOS_DECODERS="usb dcc" / OOS_DECODERS_OFF="avionics" ./build.sh    fewer decoders (tools/decoder-features.sh)
set -eu
cd "$(dirname "$0")"
root=$(cd .. && pwd)
. "$root/tools/output.sh"
export PATH="$HOME/.cargo/bin:$PATH"
[ -f assets/font.bin ] || python3 tools/mkfont.py assets/font.bin normal
[ -f assets/font-large.bin ] || python3 tools/mkfont.py assets/font-large.bin large
[ -f assets/font-larger.bin ] || python3 tools/mkfont.py assets/font-larger.bin larger
feat=$(tools/decoder-features.sh)
# shellcheck disable=SC2086 # the decoder feature arguments split into words on purpose
python3 "$root/tools/output.py" build -p oos-ui --release --target aarch64-unknown-linux-musl $feat
# shellcheck disable=SC2086
python3 "$root/tools/output.py" build -p oos-ui --release $feat
ls -l "$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/release/oos-ui"
if [ "${1:-}" = test ]; then
	out=$(mktemp -d "$CARGO_TARGET_DIR/test-shots.XXXXXX")
	mkdir -p "$out/store"
	for t in tests/*.txt; do
		python3 -I -c 'from pathlib import Path; import sys; p=Path(sys.argv[2]); (p/"script.txt").write_bytes(Path(sys.argv[1]).read_bytes().replace(b"$S", bytes(p)))' "$t" "$out"
		"$CARGO_TARGET_DIR/release/oos-ui" --store-dir "$out/store" --script "$out/script.txt"
	done
	python3 -I -c 'import glob, sys; from PIL import Image; [Image.open(f).save(f[:-4]+".png") for f in glob.glob(sys.argv[1]+"/*.ppm")]' "$out"
	ls "$out"/*.png
fi
