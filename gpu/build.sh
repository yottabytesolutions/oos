#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build.sh - the GPU part of the image (System > Graphics = GPU, gpu/README.md), in Docker, into $CARGO_TARGET_DIR/gpu/gpu/:
#   bin/oos-gpu                       the helper (gpu/oos-gpu.c)
#   usr/lib/aarch64-linux-gnu/...     Mesa with only Panfrost (EGL, GLES 3, GBM), no LLVM at run time, and what it
#                                     links (libdrm, zlib, libstdc++, glibc)
#   lib/ld-linux-aarch64.so.1         glibc's loader (oos-ui itself stays static)
#   etc/licenses/                     the licences of all of it
# boot/build-dev-image.sh runs it when its bundle receipt is stale. Mesa's build (about 10 minutes) is kept in
# rootfs/out/mesa/<environment>; identities select separate caches. Cargo clean preserves this cache.
#
# Mesa's own compilers for its built-in GPU programs need LLVM at build time (two stages); the libraries do not.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
. "$root/tools/output.sh"
out="$CARGO_TARGET_DIR/gpu"
cache="$root/rootfs/out"
case "$cache/mesa/" in "$CARGO_TARGET_DIR/"*) echo "select a target outside the Mesa cache (rootfs/out)" >&2; exit 1 ;; esac
before=$(python3 "$root/tools/output.py" revision)
ver=26.0.8
deb=26.0.8-1ubuntu0.3
sum=caf1c0061a68e88dfa74967a7e780c0e85d65b6c4e334cd69095a5dc54ad78bc
image=$("$root/tools/build-env.sh" prepare gpu-build)
environment=$("$root/tools/build-env.sh" identity gpu-build)
mkdir -p "$out" "$cache/mesa/$environment"
# the source: Ubuntu's copy of the Mesa release, checked
tarball="$cache/mesa_$ver.orig.tar.xz"
if [ ! -f "$tarball" ]; then
	docker run --rm --pull=never --platform linux/arm64 -e OUT_OWNER="$(id -u):$(id -g)" -v "$cache:/cache" $image sh -euc "
		cd /tmp && apt-get update >/dev/null && apt-get source --download-only mesa=$deb >/dev/null
		cp mesa_$ver.orig.tar.xz /cache/ && chown \"\$OUT_OWNER\" /cache/mesa_$ver.orig.tar.xz"
fi
echo "$sum  $tarball" | { shasum -a 256 -c - 2>/dev/null || sha256sum -c -; } >/dev/null || { echo "wrong checksum: $tarball" >&2; rm -f "$tarball"; exit 1; }
rm -rf "$out/gpu"
mkdir -p "$out/gpu"
docker run --rm --pull=never --platform linux/arm64 -e OUT_OWNER="$(id -u):$(id -g)" -e VER="$ver" -e ENVIRONMENT="$environment" -v "$out:/out" -v "$cache:/cache" -v "$here:/g:ro" $image sh -euc '
	W=/w; mkdir -p $W; cd $W
	L=usr/lib/aarch64-linux-gnu
	M=/cache/mesa/$ENVIRONMENT
	if [ ! -f $M/$L/libEGL.so.1 ]; then
	export PATH="/usr/lib/llvm-21/bin:$PATH"
	tar -xJf /cache/mesa_$VER.orig.tar.xz
	S=$W/mesa-$VER
	common="--buildtype=release -Db_ndebug=true -Dstrip=true -Dvulkan-drivers= -Dplatforms= -Dgles1=disabled -Dgles2=enabled
	  -Dopengl=false -Dglx=disabled -Dglvnd=disabled -Dvalgrind=disabled -Dlibunwind=disabled -Dzstd=disabled
	  -Dxmlconfig=disabled -Dgallium-va=disabled -Dvideo-codecs= -Dtools= -Dbuild-tests=false -Dgallium-drivers=panfrost"
	meson setup $W/tools $S --prefix=$W/tool-inst $common -Degl=disabled -Dgbm=disabled \
	  -Dllvm=enabled -Dmesa-clc=enabled -Dinstall-mesa-clc=true -Dprecomp-compiler=enabled -Dinstall-precomp-compiler=true \
	  > $W/meson-tools.log 2>&1 || { tail -30 $W/meson-tools.log; exit 1; }
	ninja -C $W/tools install > $W/ninja-tools.log 2>&1 || { tail -30 $W/ninja-tools.log; exit 1; }
	export PATH="$W/tool-inst/bin:$PATH"
	meson setup $W/build $S --prefix=/usr --libdir=lib/aarch64-linux-gnu $common -Degl=enabled -Dgbm=enabled \
	  -Dllvm=disabled -Dmesa-clc=system -Dprecomp-compiler=system > $W/meson.log 2>&1 || { tail -30 $W/meson.log; exit 1; }
	ninja -C $W/build > $W/ninja.log 2>&1 || { tail -30 $W/ninja.log; exit 1; }
	rm -rf $M; mkdir -p $M
	DESTDIR=$M ninja -C $W/build install > /dev/null
	cp $S/docs/license.rst $M/license.rst
	fi
	R=/out/gpu
	mkdir -p $R/bin $R/lib $R/$L $R/etc/licenses
	gcc -std=gnu11 -O2 -Wall -Wextra -I$M/usr/include -o $R/bin/oos-gpu /g/oos-gpu.c -L$M/$L -lEGL -lGLESv2 -Wl,-rpath-link,$M/$L
	strip $R/bin/oos-gpu
	cp -a $M/$L/libEGL.so.1* $M/$L/libGLESv2.so.2* $M/$L/libgbm.so.1* $M/$L/libgallium-*.so $R/$L/
	# what Mesa and the helper link from the system, with glibc and its loader
	for f in $R/bin/oos-gpu $R/$L/*.so*; do LD_LIBRARY_PATH=$R/$L ldd "$f" 2>/dev/null; done | awk "/=> \// {print \$3}" | sort -u | while read -r l; do
		case "$l" in /out/*|/cache/*) ;; *) cp -L "$l" $R/$L/ ;; esac
	done
	cp -L /lib/ld-linux-aarch64.so.1 $R/lib/
	cp $M/license.rst $R/etc/licenses/mesa
	cp /usr/share/doc/libc6/copyright $R/etc/licenses/glibc
	cp /usr/share/doc/libstdc++6/copyright $R/etc/licenses/libstdc++
	cp /usr/share/doc/libdrm2/copyright $R/etc/licenses/libdrm
	cp /usr/share/doc/zlib1g/copyright $R/etc/licenses/zlib
	chown -R "$OUT_OWNER" $R
	chown -R "$OUT_OWNER" $M
	du -sh $R; ls $R/$L
'

"$root/tools/build-env.sh" provenance gpu-build --image "$image" --output "$out/build-packages.txt"

python3 "$root/tools/output.py" stamp "$out/gpu" "$before"
