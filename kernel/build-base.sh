#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build-base.sh - build the OOS kernel from kernel/ alone: the base source, patches in `series` order, board device
# trees, firmware, config, xdma. Inside Docker, results in kernel/out/base/.
#
# Why Docker: the kernel tree has file names that differ only in case; macOS' default file system is case-insensitive.
# Everything happens in the a worktree-local Docker volume. The container has the host's architecture: on an arm64 host
# (Apple silicon, Linux arm64) the "cross" compiler is Ubuntu's native gcc under its triplet name, on any other host
# (x86-64 Linux) Ubuntu's gcc-aarch64-linux-gnu, the same gcc version. No emulation is needed for the kernel.
# The fixed time stamp and the names below make the build repeatable.
#
#   kernel/build-base.sh            build (about 5 minutes on an M-series Mac)
#   kernel/build-base.sh shell      open a shell in the build container
#   kernel/build-base.sh image      only make sure the build image is there and print its name (boot/build-dev-image.sh
#                                   compiles oos-hw with it)
#   kernel/build-base.sh purge      remove the local kernel cache
#   kernel/build-base.sh cache-dir  print its location
set -eu
here=$(cd "$(dirname "$0")" && pwd)
tarball="$here/src/linux-6.12.115-base.tar.xz"
root=$(cd "$here/.." && pwd)
case ${1:-} in
    ''|purge|cache-dir|key|verify)
        exec python3 -I "$root/tools/kernel_cache.py" "${1:-build}" ;;
    image|shell|_build) ;;
    *) echo "usage: kernel/build-base.sh [shell|image|purge|cache-dir|key|verify]" >&2; exit 2 ;;
esac
out=${2:-$here/out/base}
. "$root/tools/lib/portable.sh"
image=$("$root/tools/build-env.sh" prepare kernel)
platform=$("$root/tools/build-env.sh" platform kernel)
environment=$("$root/tools/build-env.sh" identity kernel)
workspace=$("$root/tools/build-env.sh" workspace)
if [ "${1:-}" = image ]; then
	echo "$image"
	exit 0
fi

[ -f "$tarball" ] || { echo "missing $tarball (kernel/README.md says where it comes from)" >&2; exit 1; }
(cd "$here" && sha256_check < base.sha256) || { echo "$tarball: wrong checksum" >&2; exit 1; }

docker volume create "oos-kbuild-$workspace-$environment" >/dev/null
mkdir -p "$out"
if [ "${1:-}" = shell ]; then
	exec docker run --rm --pull=never --platform "$platform" -it -v "oos-kbuild-$workspace-$environment:/w" -v "$here:/k:ro" -v "$out:/out" "$image" bash
fi
rm -f "$out/build-environment"
# the results belong to whoever runs the build (on Linux, Docker writes as root)
docker run --rm --pull=never --platform "$platform" -e OUT_OWNER="$(id -u):$(id -g)" -v "oos-kbuild-$workspace-$environment:/w" -v "$here:/k:ro" -v "$out:/out" "$image" sh -euc '
	aarch64-linux-gnu-gcc --version | head -1
	W=/w/work; K=$W/kernel-6.12; O=$W/kbuild-6.12-dho900; X=$W/xdma-build
	rm -rf "$K" "$O" "$X" "$W/modroot"; mkdir -p "$K" "$O"
	tar -xJf /k/src/linux-6.12.115-base.tar.xz -C "$K" --strip-components=1
	while read -r p; do
		[ -n "$p" ] || continue
		patch -p1 -s -d "$K" --no-backup-if-mismatch < "/k/patches/$p"
	done < /k/series
	cp /k/dts/*.dts "$K/arch/arm64/boot/dts/rockchip/"
	for d in /k/dts/*.dts; do
		printf "dtb-\$(CONFIG_ARCH_ROCKCHIP) += %s.dtb\n" "$(basename "$d" .dts)" >> "$K/arch/arm64/boot/dts/rockchip/Makefile"
	done
	F=$(sed -n "s/^CONFIG_EXTRA_FIRMWARE_DIR=\"\(.*\)\"/\1/p" /k/config)
	mkdir -p "$K/$F"; cp /k/firmware/regulatory.db* "$K/$F/"
	cp /k/config "$O/.config"
	CROSS=aarch64-linux-gnu-; JOBS=$(nproc)
	export KCFLAGS="-ffile-prefix-map=$W/=" KAFLAGS="-ffile-prefix-map=$W/="
	export KBUILD_BUILD_TIMESTAMP="Fri Oct 30 18:11:06 UTC 2026" KBUILD_BUILD_USER=oos KBUILD_BUILD_HOST=oos
	export KBUILD_BUILD_VERSION=1 SOURCE_DATE_EPOCH=1793383866 LOCALVERSION=+
	# the Rockchip drivers ask git for their revision, and this tree is none: tools/git answers from a recorded list
	export OOS_WORK="$W" PATH="/k/tools:$PATH"
	make -C "$K" O="$O" ARCH=arm64 CROSS_COMPILE=$CROSS olddefconfig
	make -C "$K" O="$O" ARCH=arm64 CROSS_COMPILE=$CROSS -j"$JOBS" Image dtbs modules > "$W/make.log" 2>&1 || { tail -n 20 "$W/make.log"; exit 1; }
	make -C "$K" O="$O" ARCH=arm64 CROSS_COMPILE=$CROSS INSTALL_MOD_PATH="$W/modroot" INSTALL_MOD_STRIP=1 modules_install >/dev/null
	cp -r /k/modules/xdma "$X"
	make -C "$O" ARCH=arm64 CROSS_COMPILE=$CROSS M="$X/xdma" modules > "$W/xdma.log" 2>&1 || { tail -n 20 "$W/xdma.log"; exit 1; }
	make -C "$O" ARCH=arm64 CROSS_COMPILE=$CROSS M="$X/xdma" INSTALL_MOD_PATH="$W/modroot" INSTALL_MOD_STRIP=1 modules_install >/dev/null
	grep -qx "CONFIG_PCIE_ROCKCHIP_HOST=m" "$O/.config"
	grep -qx "CONFIG_DWMAC_ROCKCHIP=m" "$O/.config"
	R=$(cat "$O/include/config/kernel.release"); M="$W/modroot/lib/modules/$R"
	for m in kernel/drivers/pci/controller/pcierockchiphost.ko updates/xdma.ko kernel/drivers/net/ethernet/stmicro/stmmac/dwmac-rockchip.ko; do [ -f "$M/$m" ]; done
	cp "$O/arch/arm64/boot/Image" /out/Image
	# the board and its opt-in variants (rk3399-rigol-dho900-uhs.dtb: boot card in UHS-I)
	for d in /k/dts/*.dts; do cp "$O/arch/arm64/boot/dts/rockchip/$(basename "$d" .dts).dtb" /out; done
	cp "$O/.config" /out/config
	printf "%s\n" "$R" > /out/kernel.release
	tar -C "$W/modroot" -czf /out/modules.tar.gz .
	sha256sum /out/Image /out/*.dtb > /out/SHA256SUMS
	chown -R "$OUT_OWNER" /out
	ls -la /out
'

"$root/tools/build-env.sh" provenance kernel --platform "$platform" --image "$image" --output "$out/build-packages.txt"

# Only a completed build may identify the compiler environment of these cached outputs.
printf '%s\n' "$environment" > "$out/build-environment.pending"
mv "$out/build-environment.pending" "$out/build-environment"
