#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build-dev-image.sh - the first OOS development boot image for `recovery` (Phase 2).
#
#   boot/build-dev-image.sh            -> $CARGO_TARGET_DIR/image/oos-dev.img (+ .sha256)
#   OOS_DECODERS="usb dcc" / OOS_DECODERS_OFF="avionics" boot/build-dev-image.sh   fewer decoders in oos-ui
#   (ui/tools/decoder-features.sh; the seven core decoders are always in)
#
# Contents: the kernel and DHO900 DTB from kernel/out/base (kernel/build-base.sh), the DTB's bootargs
# changed to log on the panel (tty0) and UART2, and an initramfs from rootfs/initramfs with a static busybox and the
# modules for later FPGA bring-up (pcierockchiphost, xdma), the touch panel (edt-ft5x06), the beeper (pwm-gpio) and WLAN (rtl8xxxu),
# wpa_supplicant, oos-hw and the scope screen oos-ui (started by etc/rcS).
# The image only exists as a file; installing it is a separate, explicit step.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
k="$root/kernel/out/base"
. "$root/tools/output.sh"
. "$root/tools/lib/portable.sh"
out="$CARGO_TARGET_DIR/image"
before=$(python3 "$root/tools/output.py" revision)
# the board this image is for: rootfs/board/<OOS_BOARD>, the values come from its etc/oos-board
board="$root/rootfs/board/${OOS_BOARD:-dho}"
[ -f "$board/etc/oos-board" ] || { echo "no board $board" >&2; exit 1; }
. "$board/etc/oos-board"
[ -f "$k/Image" ] || { echo "run kernel/build-base.sh first" >&2; exit 1; }
# Check daemon availability before the kernel identity queries its architecture.
docker info >/dev/null 2>&1 || { echo "Docker is not running (macOS: open -a Docker; Linux: systemctl start docker, and be in the docker group)" >&2; exit 1; }
expected_kernel=$("$root/tools/build-env.sh" identity kernel)
[ "$(cat "$k/build-environment" 2>/dev/null)" = "$expected_kernel" ] || {
    echo "kernel compiler environment changed or is unrecorded; run kernel/build-base.sh" >&2
    exit 1
}
python3 -I "$root/tools/kernel_cache.py" verify
# Docker on Linux: be in the docker group, not root (under sudo, HOME and PATH are root's, rustup's cargo is gone, and
# the results would belong to root)
if [ "$(id -u)" = 0 ] && [ -n "${SUDO_USER:-}" ]; then
	echo "do not run this with sudo. For Docker without sudo: sudo usermod -aG docker $SUDO_USER, then log in again" >&2
	exit 1
fi
# Rust from rustup: the target's standard library and rust-lld come with it, not with most distribution packages
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo "no cargo: install Rust from https://rustup.rs" >&2; exit 1; }
if command -v rustup >/dev/null 2>&1; then
	rustup target list --installed | grep -qx aarch64-unknown-linux-musl || { echo "missing Rust target: rustup target add aarch64-unknown-linux-musl" >&2; exit 1; }
else
	echo "no rustup: a cargo from the distribution usually lacks aarch64-unknown-linux-musl and rust-lld; Rust from https://rustup.rs has them" >&2
fi
# the image's busybox, wpa_supplicant and e2fsck come from arm64 containers: on other hosts they need QEMU's binfmt
probe_image=$("$root/tools/build-env.sh" prepare alpine)
docker run --rm --pull=never --platform linux/arm64 "$probe_image" true >/dev/null 2>&1 || {
	echo "Docker cannot run arm64 containers; see manual QEMU setup in docs/development.md" >&2
	echo "(or the distribution's qemu-user-static with binfmt-support)" >&2
	exit 1
}
owner="$(id -u):$(id -g)"
ui="$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/release"
# always rebuild (incremental), so the image never carries a stale binary
feat=$("$root/ui/tools/decoder-features.sh") || exit 1
# shellcheck disable=SC2086 # the decoder feature arguments split into words on purpose
python3 "$root/tools/output.py" build -p oos-ui --release --target aarch64-unknown-linux-musl $feat || exit 1
fpgad="$ui"
python3 "$root/tools/output.py" build -p oos-fpgad --release --target aarch64-unknown-linux-musl || exit 1
# Build and verify the ARM helper just like the other consumable programs.
hw="$CARGO_TARGET_DIR/hw/oos-hw"
if ! python3 "$root/tools/output.py" verify "$hw"; then
    mkdir -p "$CARGO_TARGET_DIR/hw"
    kimage=$("$root/kernel/build-base.sh" image)
    docker run --rm --pull=never --platform "$("$root/tools/build-env.sh" platform kernel)" -e OUT_OWNER="$owner" -v "$root/tools/hw:/src:ro" -v "$CARGO_TARGET_DIR/hw:/out" "$kimage" sh -euc 'aarch64-linux-gnu-gcc -static -O2 -Wall -o /out/oos-hw /src/oos-hw.c && chown "$OUT_OWNER" /out/oos-hw'
    "$root/tools/build-env.sh" provenance kernel --image "$kimage" --output "$CARGO_TARGET_DIR/hw/oos-hw.packages.txt"
    python3 "$root/tools/output.py" stamp "$hw" "$before"
fi
mkdir -p "$out"
# arm64 whatever the host is: its busybox goes into the image. An image of another architecture under this name (left
# by an older script on an x86-64 host) is built again.
image=$("$root/tools/build-env.sh" prepare initramfs)

# WLAN: wpa_supplicant and wpa_cli (docs/wlan.md)
python3 "$root/tools/output.py" verify "$root/rootfs/out/wpa_supplicant" "$root/rootfs/out/wpa_cli" || "$root/rootfs/build-wlan-tools.sh"
# /oos is checked before it is mounted
python3 "$root/tools/output.py" verify "$root/rootfs/out/e2fsck" || "$root/rootfs/build-fsck.sh"
[ -x "$root/rootfs/out/e2fsck" ] || "$root/rootfs/build-fsck.sh"
# an update with another device tree is tried by kexec_load with that tree (docs/update.md)
python3 "$root/tools/output.py" verify "$root/rootfs/out/kexec" || "$root/rootfs/build-kexec.sh"
# System > Graphics = GPU: the helper oos-gpu and Mesa (gpu/README.md)
# Receipts reject stale helpers, including changes beyond the C file.
python3 "$root/tools/output.py" verify "$CARGO_TARGET_DIR/gpu/gpu" || "$root/gpu/build.sh"

# Board-wide dependencies; device configuration never enters the image.
[ -f "${k:?}/$OOS_KERNEL_DTB" ] || { echo "no $k/$OOS_KERNEL_DTB: run kernel/build-base.sh again" >&2; exit 1; }
# the fork's own builds from rootfs/deps.lock (docs/fork/deps.md): built again when missing, or when the manifest line
# or the builder, deps.sh helper or build environment changed since (shared receipt/stamp inputs)
DEPS_LOCK="$root/rootfs/deps.lock"
. "$root/rootfs/deps.sh"
o="$root/rootfs/out"
# the LAN-only firewall (net.allow, docs/network.md 6)
if dep_stale "$o/nft" "$root/rootfs/build-nft.sh" libmnl libnftnl nftables; then "$root/rootfs/build-nft.sh"; fi
# the SSH server and its SFTP subsystem (docs/network.md 7)
if dep_stale "$o/dropbearmulti" "$root/rootfs/build-dropbear.sh" dropbear; then "$root/rootfs/build-dropbear.sh"; fi
if dep_stale "$o/sftp-server" "$root/rootfs/build-sftp-server.sh" openssh; then "$root/rootfs/build-sftp-server.sh"; fi
python3 "$root/tools/output.py" verify "$hw" "$ui/oos-ui" "$fpgad/oos-fpgad" "$fpgad/oos-update" "$CARGO_TARGET_DIR/gpu/gpu"
build_id="$(date -u +%Y%m%d-%H%M%S) kernel $(cat "$k/kernel.release")"
docker run --rm --pull=never --platform linux/arm64 -e OUT_OWNER="$owner" -e OOS_NO_AUTOACQ="${OOS_NO_AUTOACQ:-}" -v "$root/rootfs/initramfs:/src:ro" -v "$root/keys/update.pub:/staged-keys:ro" -v "$board:/board:ro" -v "$root/rootfs/out:/wlan:ro" -v "$CARGO_TARGET_DIR/gpu/gpu:/gpu:ro" -v "$CARGO_TARGET_DIR/hw:/hw:ro" -v "$ui:/ui:ro" -v "$fpgad:/fpgad:ro" -v "$k:/k:ro" -v "$out:/out" -e BUILD_ID="$build_id" "$image" sh -euc '
	r=/tmp/root; rm -rf $r; mkdir -p $r
	. /board/etc/oos-board
	cp -a /src/. $r/
	# what belongs to the board: etc/oos-board and bin/oos-board, which rcS asks
	cp -a /board/. $r/
	mkdir -p $r/bin $r/sbin $r/etc $r/proc $r/sys $r/dev $r/tmp $r/run $r/mnt $r/root $r/lib/modules
	cp /bin/busybox $r/bin/busybox
	cp /hw/oos-hw $r/bin/oos-hw
	cp /ui/oos-ui $r/bin/oos-ui
	cp /fpgad/oos-fpgad $r/bin/oos-fpgad
	cp /fpgad/oos-update $r/bin/oos-update
	cp /staged-keys $r/etc/oos-staged-keys
	cp /wlan/wpa_supplicant /wlan/wpa_cli /wlan/e2fsck /wlan/kexec $r/bin/

	mkdir -p $r/etc/licenses; cp /wlan/wpa_supplicant.COPYING $r/etc/licenses/wpa_supplicant; cp /wlan/e2fsck.COPYING $r/etc/licenses/e2fsck; cp /wlan/kexec.COPYING $r/etc/licenses/kexec-tools
	# the GPU helper with Mesa and glibc (their own loader; oos-ui and the rest stay static)
	cp -a /gpu/. $r/
	# the LAN-only firewall (net.allow, docs/network.md 6)
	cp /wlan/nft $r/bin/
	cp /wlan/nft.COPYING $r/etc/licenses/nftables
	# the SSH server, started by rcS through oos-ssh (docs/network.md 7)
	cp /wlan/dropbearmulti $r/bin/
	for p in dropbear dropbearkey scp; do ln -sf dropbearmulti $r/bin/$p; done
	cp /wlan/dropbear.COPYING $r/etc/licenses/dropbear
	# its SFTP subsystem: Dropbear runs /bin/sftp-server (sftp, and scp without -O)
	cp /wlan/sftp-server $r/bin/
	cp /wlan/sftp-server.COPYING $r/etc/licenses/openssh-sftp-server
	ln -sf busybox $r/bin/sh
	echo "$BUILD_ID" > $r/etc/oos-build
	# rcS brings the acquisition hardware up at boot (cold boot + oos-ui --hw --afe); OOS_NO_AUTOACQ=1 leaves it out
	[ -n "$OOS_NO_AUTOACQ" ] || touch $r/etc/oos-autoacq
	mkdir -p /tmp/m && tar -C /tmp/m -xzf /k/modules.tar.gz
	rel=$(cat /k/kernel.release); M=/tmp/m/lib/modules/$rel
	mkdir -p $r/lib/modules/$rel
	named=$(for n in $OOS_MODULE_NAMES; do (cd $M && find . -name "$n.ko*" | sed "s|^\./||"); done)
	for f in $OOS_MODULES $named; do
		mkdir -p $r/lib/modules/$rel/$(dirname $f); cp $M/$f $r/lib/modules/$rel/$f
	done
	cp $M/modules.dep $M/modules.alias $M/modules.order $M/modules.builtin* $r/lib/modules/$rel/ 2>/dev/null || true
	# the console node the kernel opens before /init mounts devtmpfs
	mknod -m 600 $r/dev/console c 5 1
	# copies a file sync made in the build outputs (iCloud: "libc.so.6 2", "oos-gpu 2") stay out of the image: 27 MB
	# of them in rootfs/out/gpu made the initramfs of the first 0.1.13 build 10 MB larger (2026-11-15)
	dup=$(cd $r && find . \( -name "* [0-9]" -o -name "* [0-9].*" \) | head -5)
	[ -z "$dup" ] || { echo "build-dev-image.sh: copies of a file sync in the image, remove them from the source: $dup" >&2; exit 1; }
	(cd $r && find . | sort | cpio -o -H newc -R 0:0 --quiet) > /tmp/root.cpio
	gzip -9n < /tmp/root.cpio > /out/initramfs.cpio.gz
	cp /k/$OOS_KERNEL_DTB /out/$OOS_IMAGE_NAME.dtb
	fdtput -t s /out/$OOS_IMAGE_NAME.dtb /chosen bootargs "$OOS_BOOTARGS"
	fdtget /out/$OOS_IMAGE_NAME.dtb /chosen bootargs
	# the rebuilt BusyBox and its build glibc: their sources go with a release
	# tools/release-sources.sh reads the build glibc from this file
	cat /usr/local/share/oos-busybox.version > /out/busybox.version
	chown -R "$OUT_OWNER" /out
	ls -la /out
'
# Invalidate metadata and install files before replacing the image. A failed build must not leave them usable.
rm -f "$out/oos-dev.models" "$out/oos-dev.build" "$out/oos-dev.oosupd" "$out/${OOS_GEL_STEM}_Install.GEL"
python3 -I "$here/oos-bootimg.py" build --kernel "$k/Image" --dtb "$out/$OOS_IMAGE_NAME.dtb" \
	--ramdisk "$out/initramfs.cpio.gz" --name "$OOS_IMAGE_NAME" --out "$out/oos-dev.img"
(cd "$out" && echo "$(sha256 oos-dev.img)  oos-dev.img" > oos-dev.img.sha256 && cat oos-dev.img.sha256)
printf '%s\n' "$build_id" > "$out/oos-dev.build"
"$root/tools/build-env.sh" provenance initramfs --image "$image" --output "$out/initramfs.packages.txt"
python3 "$root/tools/output.py" stamp "$out/oos-dev.img" "$before"
python3 "$root/tools/output.py" stamp "$out/oos-dev.build" "$before"
cp "$board/etc/oos-models" "$out/oos-dev.models"
python3 "$root/tools/output.py" stamp "$out/oos-dev.models" "$before"
echo "image built. Sign explicitly with boot/sign-dev-image.sh, then package with boot/build-gel.py (docs/update.md)."
