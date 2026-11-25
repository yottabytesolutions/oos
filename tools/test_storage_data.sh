#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
# Exercise first-boot formatting, orderly shutdown and repeat startup with the image's filesystem tools.
# Requires the initramfs builder, rootfs/build-fsck.sh outputs, an ARM oos-update build and Docker loop mounts.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/tools/output.sh"
w=$(mktemp -d "$CARGO_TARGET_DIR/storage-data.XXXXXX")
trap 'rm -rf "$w"' EXIT
trap 'exit 1' HUP INT TERM
image=$("$root/tools/build-env.sh" resolve initramfs)
cp "$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/debug/oos-update" "$w/oos-update"
cp "$root/rootfs/out/e2fsck" "$w/e2fsck"
cp "$root/rootfs/out/tune2fs" "$w/tune2fs"
cp "$root/rootfs/initramfs/bin/oos-shutdown-data" "$w/shutdown"
cp "$root/rootfs/initramfs/bin/oos-mount-data" "$w/mount-data"
cp "$root/rootfs/board/${OOS_BOARD:-dho}/etc/oos-board" "$w/oos-board"
docker run --rm --pull=never --network=none --platform linux/arm64 "$image" cat /bin/busybox > "$w/busybox"
chmod +x "$w/busybox"
python3 - "$root" "$w" <<'PY'
import hashlib
from pathlib import Path
import sys
sys.path.insert(0, str(Path(sys.argv[1]) / 'boot'))
import boardconf
board = boardconf.load()
start = int(board['OOS_OOSDATA_LBA']) * 512
size = int(board['OOS_OOSDATA_SECTORS']) * 512
parm = int(board['OOS_PARM_LBA']) * 512
boot = int(board['OOS_BOOT_LBA']) * 512
work = Path(sys.argv[2])
(work / 'start').write_text(f'{start // 512}\n')
(work / 'size').write_text(f'{size // 512}\n')
with (work / 'card').open('wb') as output:
    output.truncate(start + size + 512)
    output.seek(parm)
    output.write(b'PARM ' + board['OOS_PARM_MARKS'].encode()
                 + b' 0x00020000@0x0002c000(recovery) 0x00038000@0x0004c000(backup)')
    output.seek(boot)
    output.write(b'KRNL')
    output.seek(start - 512)
    output.write(bytes([91]) * 512)
    output.seek(start + size)
    output.write(bytes([92]) * 512)
with (work / 'card').open('rb') as source:
    before = hashlib.sha256(source.read(start)).hexdigest()
(work / 'protected.sha256').write_text(before)
PY
docker run --rm --pull=never --network=none --platform linux/arm64 --privileged -v "$w:/w" "$image" sh -efuc '
    mkdir /tmp/storage-tools
    ln -s /w/busybox /tmp/storage-tools/mke2fs
    ln -s /w/e2fsck /tmp/storage-tools/e2fsck
    ln -s /w/tune2fs /tmp/storage-tools/tune2fs
    ln -s /w/oos-update /tmp/storage-tools/oos-update
    ln -sf /bin/busybox /bin/sync
    export PATH=/tmp/storage-tools:$PATH
    /w/oos-update storage-data --card /w/card
    /w/oos-update storage-data --card /w/card
    status=$(/w/oos-update status --quick --card /w/card)
    printf "%s\n" "$status" | grep -Fx "data journal enabled"

	# Only the scratch file is attached, at the data area offset and size.
	loop=$(losetup --find --show --offset "$(($(cat /w/start) * 512))" --sizelimit "$(($(cat /w/size) * 512))" /w/card)
	trap "losetup -d $loop" EXIT
	export OOS_BOARD_FILE=/w/oos-board
	sed -e "s|/sys/class/block/mmcblk0p1|/w|g" -e "s|/dev/mmcblk0p1|$loop|g" \
		-e "s|/dev/mmcblk0|/w/card|g" /w/mount-data > /w/startup
	sed -i "1c#!/bin/busybox sh" /w/shutdown
	chmod +x /w/shutdown
	/bin/busybox sh /w/startup 1
	for attempt in first second; do
		[ ! -e /tmp/oos-data.closing ]
		printf "%s\n" "$attempt" >> /oos/probe
		receipt=$(/w/shutdown ui-drained)
		grep -q "^oos-closed /oos tmpfs ro[, ]" /proc/mounts
		if touch /oos/late 2>/w/late-error; then exit 1; fi
		grep -F "Read-only file system" /w/late-error
		if grep -q " /oos ext4 " /proc/mounts; then exit 1; fi
		if [ "$attempt" = first ]; then
			set -- $receipt
			[ "$1" = mount ]
			/w/shutdown restore "" "$2" "$3" "$4"
			if grep -q "^oos-closed /oos " /proc/mounts; then exit 1; fi
			[ "$(cat /oos/probe)" = first ]
		fi
	done
	rmdir /tmp/oos-data.closing
	before=$(sha256sum /w/card)
	repair=$(/w/oos-update storage-data --card /w/card)
	test -z "$repair"
	test "$(sha256sum /w/card)" = "$before"
	/bin/busybox umount /oos
	/bin/busybox umount /oos
	/bin/busybox sh /w/startup 1
	[ ! -e /tmp/oos-data.notice ]
	grep -Fx second /oos/probe
	/w/shutdown ui-drained
'
python3 - "$root" "$w" <<'PY'
import hashlib
from pathlib import Path
import sys
sys.path.insert(0, str(Path(sys.argv[1]) / 'boot'))
import boardconf
board = boardconf.load()
start = int(board['OOS_OOSDATA_LBA']) * 512
size = int(board['OOS_OOSDATA_SECTORS']) * 512
work = Path(sys.argv[2])
with (work / 'card').open('rb') as source:
    assert hashlib.sha256(source.read(start)).hexdigest() == (work / 'protected.sha256').read_text()
    data = source.read(size)
    assert data[1080:1082] == b'\x53\xef', 'missing ext superblock'
    assert data[1144:1160].rstrip(b'\0') == b'oosdata', 'missing data label'
    assert [int.from_bytes(data[offset:offset + 4], 'little') for offset in (1116, 1120, 1124)] == [0x24, 2, 1]
    assert source.read() == bytes([92]) * 512, 'write beyond OOS data'
print('storage-data: first boot and clean restart passed without journal replay or repair')
PY
