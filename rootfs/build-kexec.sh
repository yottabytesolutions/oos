#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build-kexec.sh - a static kexec (kexec-tools) for the image, in Docker, into rootfs/out/. boot/build-dev-image.sh runs
# it when it is missing. oos-update starts an update whose device tree differs from the running one with it: the
# kexec_file_load call it uses otherwise gives the new kernel the running tree, kexec_load (`kexec -c --dtb`) takes the
# new one (docs/update.md, "Trial start by kexec").
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/out"
root=$(cd "$here/.." && pwd)
image=$("$root/tools/build-env.sh" prepare rootfs-kexec)
before=$(python3 "$root/tools/output.py" identity "$out/kexec")
. "$here/deps.sh"
ver=$(dep_ver kexec-tools)
dep_fetch kexec-tools >/dev/null
mkdir -p "$out"
# arm64 containers: on an x86-64 host through QEMU (boot/build-dev-image.sh says how); the results belong to the caller
docker run --rm --pull=never --platform linux/arm64 -e OUT_OWNER="$(id -u):$(id -g)" -e VER="$ver" -v "$out:/out" -v "$DEPS_SOURCES:/src:ro" "$image" sh -euc '
	cd /tmp && tar -xJf /src/kexec-tools-$VER.tar.xz && cd kexec-tools-$VER
	./configure --without-zlib --without-lzma --without-xen CFLAGS=-Os LDFLAGS=-static >/tmp/conf.log 2>&1 || { tail -20 /tmp/conf.log; exit 1; }
	make -j"$(nproc)" >/tmp/build.log 2>&1 || { tail -30 /tmp/build.log; exit 1; }
	strip build/sbin/kexec
	cp build/sbin/kexec /out/kexec
	cp COPYING /out/kexec.COPYING
	chown "$OUT_OWNER" /out/kexec /out/kexec.COPYING
'
ls -la "$out/kexec"

"$root/tools/build-env.sh" provenance rootfs-kexec --image "$image" --output "$out/kexec.packages.txt"

python3 "$root/tools/output.py" stamp "$out/kexec" "$before"
