#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build-fsck.sh - a static e2fsck for the image, in Docker, into rootfs/out/. boot/build-dev-image.sh runs it when it is
# missing. The scope is switched off at the mains, so /oos is checked before every mount (oos-mount-data).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/out"
root=$(cd "$here/.." && pwd)
image=$("$root/tools/build-env.sh" prepare rootfs-fsck)
before=$(python3 "$root/tools/output.py" identity "$out/e2fsck")
. "$here/deps.sh"
ver=$(dep_ver e2fsprogs)
dep_fetch e2fsprogs >/dev/null
mkdir -p "$out"
# arm64 containers: on an x86-64 host through QEMU (boot/build-dev-image.sh says how); the results belong to the caller
docker run --rm --pull=never --platform linux/arm64 -e OUT_OWNER="$(id -u):$(id -g)" -e VER="$ver" -v "$out:/out" -v "$DEPS_SOURCES:/src:ro" "$image" sh -euc '
	cd /tmp && tar -xJf /src/e2fsprogs-$VER.tar.xz && cd e2fsprogs-$VER
	./configure --disable-nls --disable-uuidd --disable-fuse2fs --disable-defrag --disable-imager --disable-resizer CFLAGS=-Os >/tmp/conf.log 2>&1 || { tail -20 /tmp/conf.log; exit 1; }
	make -j"$(nproc)" libs >/tmp/build.log 2>&1 || { tail -30 /tmp/build.log; exit 1; }
	make -C e2fsck -j"$(nproc)" e2fsck.static >>/tmp/build.log 2>&1 || { tail -30 /tmp/build.log; exit 1; }
	strip e2fsck/e2fsck.static
	cp e2fsck/e2fsck.static /out/e2fsck
	cp NOTICE /out/e2fsck.COPYING
	chown "$OUT_OWNER" /out/*
'
ls -la "$out/e2fsck"

"$root/tools/build-env.sh" provenance rootfs-fsck --image "$image" --output "$out/e2fsck.packages.txt"

python3 "$root/tools/output.py" stamp "$out/e2fsck" "$before"
