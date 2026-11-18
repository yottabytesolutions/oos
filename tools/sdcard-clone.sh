#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# sdcard-clone.sh - clone the scope's boot SD card on macOS or Linux, and write a clone to the second card.
#
#   sdcard-clone.sh list                       list SD and USB disks
#   sdcard-clone.sh read  DISK out.img         card -> image (+ out.img.sha256, + layout report)
#   sdcard-clone.sh write in.img DISK          refused: whole-card writes include protected storage
#   sdcard-clone.sh verify in.img DISK         compare a card with an image (sha256 over the image length)
#   sdcard-clone.sh fit in.img DISK|BYTES out.img
#                                              a copy of a card image for a smaller card: the ext4 of the last
#                                              partition (userdata) shrunk to what is left, in Docker. in.img is only read
# DISK is a whole disk: diskN on macOS, sdX or mmcblkN on Linux.
# Safety: refuses the disk of the running system and any disk that is not removable/external SD or USB media;
# refuses to write an image that is larger than the card (fit makes one that is not); never runs without an explicit DISK.
set -eu

die() { echo "error: $*" >&2; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib/portable.sh"

case $(uname -s) in
Darwin)
	info() { diskutil info "/dev/$1" 2>/dev/null; }
	field() { info "$1" | awk -F': *' -v k="$2" '$1 ~ "^ *"k"$" {print $2; exit}'; }
	check_disk() {
		d=$1
		case "$d" in disk[0-9]*) ;; *) die "give a whole disk like disk4, not '$d'";; esac
		case "$d" in *s[0-9]*) die "'$d' is a partition, give the whole disk";; esac
		info "$d" >/dev/null || die "no such disk: $d"
		boot=$(diskutil info / | awk -F': *' '/Part of Whole/ {print $2}')
		[ "$d" != "$boot" ] || die "$d holds the running macOS"
		proto=$(field "$d" "Protocol")
		removable=$(field "$d" "Removable Media")
		location=$(field "$d" "Device Location")
		virtual=$(field "$d" "Virtual")
		[ "$virtual" != "Yes" ] || die "$d is a virtual disk (disk image)"
		case "$proto" in
			"Secure Digital"|USB) ;;
			*) die "$d protocol is '$proto' (expected Secure Digital or USB)";;
		esac
		case "$removable$location" in
			*Removable*|*External*) ;;
			*) die "$d is neither removable nor external ($removable / $location)";;
		esac
	}
	disk_bytes() { field "$1" "Disk Size" | sed -E 's/.*\(([0-9]+) Bytes\).*/\1/'; }
	describe() {
		d=$1
		printf "%s: %s, %s bytes, protocol %s, %s\n" "$d" "$(field "$d" "Device / Media Name")" "$(disk_bytes "$d")" \
			"$(field "$d" "Protocol")" "$(field "$d" "Removable Media")"
	}
	media() { case "$(field "$1" "Protocol")" in "Secure Digital"|USB) return 0;; esac; return 1; }
	disks() { diskutil list physical | awk '/^\/dev\/disk/ {print $1}' | sed 's|/dev/||'; }
	raw() { echo "/dev/r$1"; }
	unmount() { diskutil unmountDisk "/dev/$1" >/dev/null; }
	eject_disk() { diskutil eject "/dev/$1" >/dev/null && echo "ejected $1"; }
	;;
Linux)
	prop() { lsblk -dnbo "$2" "/dev/$1" 2>/dev/null | sed 's/^ *//; s/ *$//'; }
	# the disk, its partitions and what sits on them (LVM, dm-crypt), innermost first
	devices() { lsblk -nrpo NAME "/dev/$1" | awk '{ l[NR] = $0 } END { for (i = NR; i > 0; i--) print l[i] }'; }
	# every mount point of each of them (findmnt lists them all, also a second mount of one file system) and swap
	mounts() {
		devices "$1" | while read -r n; do findmnt -nr -o TARGET -S "$n" || true; done
		lsblk -nro MOUNTPOINT "/dev/$1" | grep -x '\[SWAP\]' || true
	}
	# an SD card in a reader: mmc with card type SD (eMMC is type MMC and is a system disk), or a USB reader or stick
	kind() {
		case "$1" in
			mmcblk*) [ "$(cat "/sys/block/$1/device/type" 2>/dev/null)" = SD ] && echo SD || echo "MMC (eMMC)";;
			*) t=$(prop "$1" TRAN); [ "$t" = usb ] && echo USB || echo "${t:-unknown}";;
		esac
	}
	check_disk() {
		d=$1
		case "$d" in
			sd[a-z]|sd[a-z][a-z]|mmcblk[0-9]|mmcblk[0-9][0-9]) ;;
			*) die "give a whole disk like sdb or mmcblk0, not '$d'";;
		esac
		[ "$(prop "$d" TYPE)" = disk ] || die "no such disk: $d"
		# only a desktop's mounts of removable media pass: not /, not swap, not a live system's boot medium (/cdrom,
		# /run/live/medium)
		m=$(mounts "$d" | grep -vE '^/media/|^/run/media/' | head -n 1)
		[ -z "$m" ] || die "$d is mounted at $m and may hold the running Linux; unmount it if it does not"
		k=$(kind "$d")
		case "$k" in SD|USB) ;; *) die "$d is '$k' (expected an SD card or USB media)";; esac
	}
	disk_bytes() { prop "$1" SIZE; }
	describe() {
		d=$1
		printf "%s: %s, %s bytes, %s, removable %s\n" "$d" "$(prop "$d" MODEL)" "$(disk_bytes "$d")" "$(kind "$d")" \
			"$(prop "$d" RM)"
	}
	media() { case "$(kind "$1")" in SD|USB) return 0;; esac; return 1; }
	disks() { lsblk -dnro NAME,TYPE | awk '$2 == "disk" {print $1}'; }
	raw() { echo "/dev/$1"; }
	unmount() {
		devices "$1" | while read -r n; do
			if findmnt -S "$n" >/dev/null; then sudo umount --all-targets "$n" || exit 1; fi
		done
	}
	eject_disk() {
		sync
		if command -v eject >/dev/null 2>&1; then sudo eject "/dev/$1" && echo "ejected $1"; else echo "done; remove $1"; fi
	}
	;;
*)
	die "sdcard-clone.sh supports macOS and Linux, not $(uname -s)";;
esac


cmd=${1:-}
case "$cmd" in
list)
	disks | while read -r d; do
		if media "$d"; then describe "$d"; fi
	done
	;;
read)
	[ $# -eq 3 ] || die "usage: $0 read DISK out.img"
	d=$2; out=$3
	check_disk "$d"; describe "$d"
	[ ! -e "$out" ] || die "$out exists, refusing to overwrite"
	bytes=$(disk_bytes "$d")
	free=$(df -Pk "$(dirname "$out")" | awk 'NR==2 {print $4 * 1024}')
	[ "$free" -gt "$bytes" ] || die "not enough free space for $bytes bytes next to $out"
	unmount "$d"
	echo "reading $(raw "$d") -> $out ($bytes bytes)"
	if command -v pv >/dev/null 2>&1; then
		sudo sh -c 'pv -s "$1" "$2" > "$3"' sh "$bytes" "$(raw "$d")" "$out"
	else
		sudo dd if="$(raw "$d")" of="$out" bs=4194304 status=progress
	fi
	sudo chown "$(id -u):$(id -g)" "$out"
	[ "$(file_bytes "$out")" -eq "$bytes" ] || die "image size differs from the card size"
	echo "hashing image and card ..."
	h_img=$(sha256 "$out")
	h_dev=$(sudo dd if="$(raw "$d")" bs=4194304 2>/dev/null | sha256)
	[ "$h_img" = "$h_dev" ] || die "sha256 mismatch: image $h_img, card $h_dev"
	echo "$h_img  $(basename "$out")" > "$out.sha256"
	echo "ok: sha256 $h_img"
	py="$here/.venv/bin/python"; [ -x "$py" ] || py=python3
	"$py" "$here/sdcard-layout.py" "$out" | tee "$out.layout.txt"
	;;
write)
    die "whole-card writes include protected storage (ADR 0011); use boot/card-install.py for recovery";;
verify)
	[ $# -eq 3 ] || die "usage: $0 verify in.img DISK"
	img=$2; d=$3
	check_disk "$d"
	ib=$(file_bytes "$img")
	unmount "$d" 2>/dev/null || true
	if [ -f "$img.sha256" ] && [ "${IMAGE_VERIFIED:-0}" = 1 ]; then h_img=$(cut -d' ' -f1 "$img.sha256"); else h_img=$(sha256 "$img"); fi
	h_dev=$(sudo dd if="$(raw "$d")" bs=4194304 count=$(( (ib + 4194303) / 4194304 )) 2>/dev/null | head -c "$ib" | sha256)
	[ "$h_img" = "$h_dev" ] && echo "verify ok: $h_img" || die "verify FAILED: image $h_img, card $h_dev"
	;;
fit)
	[ $# -eq 4 ] || die "usage: $0 fit in.img DISK|BYTES out.img"
	img=$2; target=$3; out=$4
	[ -f "$img" ] || die "no image $img"
	[ ! -e "$out" ] || die "$out exists, refusing to overwrite"
	case "$target" in
		""|*[!0-9]*) check_disk "$target"; tb=$(disk_bytes "$target");;
		*) tb=$target;;
	esac
	ib=$(file_bytes "$img")
	[ "$ib" -gt "$tb" ] || die "the image fits as it is ($ib <= $tb bytes): $0 write $img DISK"
	# the last partition of the stock table (PARM at LBA 8192, offsets relative to it): it has to run to the end of the
	# card ("-"), so a smaller card only makes it shorter, and it has to be the only one past the new end
	last=$(python3 -I - "$img" <<'EOF'
import re, struct, sys
f = open(sys.argv[1], "rb"); f.seek(8192 * 512); raw = f.read(64 * 512)
if raw[:4] != b"PARM":
    sys.exit("no PARM block at LBA 8192: not a DHO card image")
txt = raw[8:8 + struct.unpack_from("<I", raw, 4)[0]].decode("latin1")
m = re.search(r"mtdparts=[^:]+:(\S+)", txt)
parts = [re.match(r"(-|0x[0-9a-fA-F]+)@(0x[0-9a-fA-F]+)\(([^):]+)", p) for p in m.group(1).split(",")] if m else []
if not parts or not all(parts):
    sys.exit("no readable partition table in PARM")
size, off, name = parts[-1].groups()
if size != "-":
    sys.exit(f"the last partition ({name}) has a fixed size, it does not end with the card")
print(name, 8192 + int(off, 16))
EOF
) || die "cannot read the partition table of $img"
	name=${last% *}; lba=${last#* }
	off=$((lba * 512))
	# whole 4 MiB below the end of the new card
	avail=$(( (tb - off) / 4194304 * 4194304 ))
	[ "$avail" -gt 0 ] || die "$name starts at $off bytes, past the end of $tb bytes"
	free=$(df -Pk "$(dirname "$out")" | awk 'NR==2 {print $4 * 1024}')
	[ "$free" -gt "$tb" ] || die "not enough free space for $tb bytes next to $out"
	docker info >/dev/null 2>&1 || die "Docker is not running"
	echo "$name at LBA $lba: its ext4 shrunk to $avail bytes, for a card of $tb bytes"
	indir=$(cd "$(dirname "$img")" && pwd); outdir=$(mkdir -p "$(dirname "$out")" && cd "$(dirname "$out")" && pwd)
	# the partition is copied into a Docker volume (sparse: only what is used takes room), checked and shrunk there;
	# in.img is mounted read only
	docker run --rm -e IMG="$(basename "$img")" -e OUT="$(basename "$out")" -e OFF="$off" -e AVAIL="$avail" -e NAME="$name" \
		-e OUT_OWNER="$(id -u):$(id -g)" -v "$indir:/in:ro" -v "$outdir:/out" -v oos-sdfit:/w alpine:3.22@sha256:49340b346798875b5592e5d420241f2f9cfba5f47e62ebb1446a1b34734e898f sh -euc '
		apk add --no-cache e2fsprogs e2fsprogs-extra coreutils >/dev/null
		p=/w/part; rm -f $p
		trap "rm -f $p" EXIT
		dd if="/in/$IMG" of=$p bs=4M iflag=skip_bytes skip="$OFF" conv=sparse status=none
		fsbytes() { dumpe2fs -h $p 2>/dev/null | awk -F: "/^Block count:/ {c=\$2} /^Block size:/ {s=\$2} END {print c * s}"; }
		[ "$(fsbytes)" -gt 0 ] || { echo "$NAME holds no ext2/3/4 file system"; exit 1; }
		# resize2fs wants a freshly checked file system; this changes only the copy
		rc=0; e2fsck -f -y $p >/w/fsck.log 2>&1 || rc=$?
		[ $rc -le 1 ] || { tail -20 /w/fsck.log; echo "e2fsck returned $rc on $NAME"; exit 1; }
		# 1 is usual: the stock system is switched off at the mains, its journal is replayed
		[ $rc = 0 ] || echo "e2fsck repaired the copy of $NAME, usually the journal of the last power-off (/w/fsck.log in the volume oos-sdfit)"
		if [ "$(fsbytes)" -gt "$AVAIL" ]; then
			resize2fs $p "$((AVAIL / 1024))K" >/w/resize.log 2>&1 || { cat /w/resize.log; echo "the files on $NAME need more than $AVAIL bytes"; exit 1; }
		fi
		e2fsck -f -n $p >/dev/null 2>&1 || { echo "$NAME is not clean after the resize"; exit 1; }
		n=$(fsbytes)
		dd if="/in/$IMG" of="/out/$OUT" bs=4M iflag=count_bytes count="$OFF" status=none
		dd if=$p of="/out/$OUT" bs=4M iflag=count_bytes count="$n" oflag=seek_bytes seek="$OFF" conv=sparse,notrunc status=none
		truncate -s $((OFF + n)) "/out/$OUT"
		chown "$OUT_OWNER" "/out/$OUT"
		echo "$NAME: $n bytes of file system, image $((OFF + n)) bytes"
	' || { rm -f "$out"; die "fit failed, $img is unchanged"; }
	ob=$(file_bytes "$out")
	[ "$ob" -le "$tb" ] || die "$out came out larger than the card ($ob > $tb)"
	h=$(sha256 "$out")
	echo "$h  $(basename "$out")" > "$out.sha256"
	echo "ok: $out ($ob bytes, sha256 $h). Next: $0 write $out DISK"
	;;
*)
	sed -n '2,15p' "$0"; exit 1;;
esac
