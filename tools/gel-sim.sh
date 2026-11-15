#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# gel-sim.sh - the install without opening the scope, on a file laid out like a stock card: a Linux container with the
# paths of the stock system, a USB stick folder and mksh as /system/bin/sh, running the selected image/DHO800_DHO900_OOS_Install.GEL
# through the steps of the stock update. Needs Docker and the three completed host steps:
# boot/build-dev-image.sh, boot/sign-dev-image.sh, then python3 -I boot/build-gel.py.
#
#   tools/gel-sim.sh [stock do_update.sh]    the real script of a stock update file, if at hand; else the same steps here
#
# OOS_CAL_DIR: a folder with vendor.bin, for the model check. Without it the run uses oos-any-model.txt.
# OOS_GEL_OLD: the GEL of an older build, installed first: the run then shows the update of an older build.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
sim_image=$("$root/tools/build-env.sh" resolve sim-gel)
. "$root/tools/output.sh"
gel="$CARGO_TARGET_DIR/image/DHO800_DHO900_OOS_Install.GEL"
[ -f "$gel" ] || { echo "run boot/build-dev-image.sh, boot/sign-dev-image.sh, then python3 -I boot/build-gel.py first" >&2; exit 1; }
python3 "$root/tools/output.py" verify "$gel"
w=$(mktemp -d "${TMPDIR:-/tmp}/oos-gel-sim.XXXXXX")
trap 'rm -rf "$w"' EXIT
trap 'exit 1' HUP INT TERM
mkdir -p "$w/rigol-data"
[ -z "${1:-}" ] || cp "$1" "$w/do_update.sh"
[ -n "${OOS_CAL_DIR:-}" ] && cp "$OOS_CAL_DIR/vendor.bin" "$w/rigol-data/" 2>/dev/null || true
echo calibration > "$w/rigol-data/cal_example.hex"
cp "$gel" "$w/"
[ -z "${OOS_GEL_OLD:-}" ] || cp "$OOS_GEL_OLD" "$w/old.GEL"
python3 -I - "$w/card" <<'PY'
import sys
S = 512
parm = (b"PARM\0\0\0\0FIRMWARE_VER:9.0\nCMDLINE:mtdparts=rk29xxnand:0x00002000@0x00002000(uboot),0x00002000@0x00004000(trust),"
        b"0x00002000@0x00006000(misc),0x00010000@0x0001C000(boot),0x00020000@0x0002C000(recovery),0x00038000@0x0004C000(backup)\n")
with open(sys.argv[1], "wb") as f:
    f.truncate(600000 * S)
    for lba, b in ((64, b"LOADER"), (8192, parm), (16384, b"UBOOT"), (24576, b"TRUST"),
                   (122880, b"KRNL\0\0\x10\0stock boot" + bytes(range(256)) * 40), (188416, b"KRNL\0\0\x20\0stock recovery")):
        f.seek(lba * S); f.write(b)
PY

docker run --rm --pull=never --network=none --platform linux/arm64 -v "$w:/w" "$sim_image" bash -euc '
	mkdir -p /system/bin /rigol/shell /mnt/media_rw/ABCD-1234 /rigol
	ln -s /usr/bin/mksh /system/bin/sh; ln -s /bin/bash /system/bin/bash
	# the busybox of the stock system unpacks gzip by itself
	printf "#!/bin/sh\nshift\nexec tar \"\$@\"\n" > /system/bin/busybox; chmod +x /system/bin/busybox
	cp -r /w/rigol-data /rigol/data
	if [ -f /w/do_update.sh ]; then cp /w/do_update.sh /rigol/shell/; else cat > /rigol/shell/do_update.sh <<UPD
#!/system/bin/bash
mkdir -p /mnt/tmp /mnt/bak; cd /mnt/tmp
/system/bin/busybox tar -xvf "\$1" >/dev/null 2>&1 || exit 1
/mnt/tmp/shell/do_extract.sh "\$2"
UPD
	fi
	chmod +x /rigol/shell/do_update.sh
	S=/mnt/media_rw/ABCD-1234; G=DHO800_DHO900_OOS_Install.GEL; cp /w/$G $S/
	[ -f /rigol/data/vendor.bin ] || touch $S/oos-any-model.txt
	export OOS_GEL_CARD=/w/card
	magic() { dd if=/w/card bs=512 skip=$1 count=1 2>/dev/null | head -c 4; echo; }
	upd() { su -s /bin/sh -c "OOS_GEL_CARD=/w/card /rigol/shell/do_update.sh $S/$G" root; }
	sums() { dd if=/w/card bs=512 skip=$1 count=$2 2>/dev/null | sha256sum | cut -c1-16; }

	if [ -f /w/old.GEL ]; then
		echo "== an older build first"; cp /w/old.GEL $S/$G; upd >/dev/null; echo "exit $?"
		printf KRNL | dd of=/w/card bs=512 seek=122880 conv=notrunc 2>/dev/null; cp /w/$G $S/
	fi
	echo "== dry run"; touch $S/oos-dry-run.txt; h0=$(sha256sum < /w/card); upd; echo "exit $?"; rm $S/oos-dry-run.txt
	[ "$h0" = "$(sha256sum < /w/card)" ] || { echo "FAIL: dry run changed card"; exit 1; }
	echo "card unchanged"
	[ -s $S/OOS-rescue/SHA256SUMS ] || { echo "FAIL: missing rescue checksums"; exit 1; }
	(cd $S/OOS-rescue && sha256sum -c SHA256SUMS)
	cmp /rigol/data/cal_example.hex $S/OOS-rescue/rigol-data/cal_example.hex
	ls /rigol/data $S/OOS-rescue/rigol-data | head -8

	echo "== install"; b=$(sums 122880 65536); upd; echo "exit $?"
	echo "boot magic: $(magic 122880)"; echo "backup magic: $(magic 319488)"
	[ "$b" = "$(sums 319488 65536)" ] || { echo "FAIL: backup differs from stock boot"; exit 1; }
	[ "$(magic 122880)" = OOSK ] && [ "$(magic 319488)" = KRNL ] || { echo "FAIL: boot/backup magic"; exit 1; }
	echo "backup = stock boot"
	echo "recovery: $(dd if=/w/card bs=512 skip=188416 count=1 2>/dev/null | head -c 8)"
	[ "$(dd if=/w/card bs=512 skip=188416 count=1 2>/dev/null | head -c 8)" = "ANDROID!" ] || { echo "FAIL: recovery image magic"; exit 1; }
	grep -E "newer|kept, only" $S/oos-install.log || true
	ls /mnt/tmp

	echo "== after a start of OOS (one-shot), the file selects it again"; printf KRNL | dd of=/w/card bs=512 seek=122880 conv=notrunc 2>/dev/null
	upd; echo "exit $?"; [ "$(magic 122880)" = OOSK ] || { echo "FAIL: one-shot reselect"; exit 1; }

	echo "== a card that is not stock"; dd if=/dev/zero of=/w/card bs=512 seek=8192 count=4 conv=notrunc 2>/dev/null
	bad_before=$(sha256sum < /w/card)
	if upd; then echo "FAIL: non-stock card accepted"; exit 1; fi
	[ "$bad_before" = "$(sha256sum < /w/card)" ] || { echo "FAIL: non-stock card changed"; exit 1; }
	echo "GEL TEST PASSED"
	echo "== log"; tail -22 $S/oos-install.log
'
