#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
# Exercise first-boot formatting and later repair with the image's BusyBox and standalone e2fsck.
# Requires the initramfs builder, rootfs/out/e2fsck and a debug ARM oos-update build.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
. "$root/tools/output.sh"
w=$(mktemp -d "$CARGO_TARGET_DIR/storage-data.XXXXXX")
trap 'rm -rf "$w"' EXIT
trap 'exit 1' HUP INT TERM
image=$("$root/tools/build-env.sh" resolve initramfs)
cp "$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/debug/oos-update" "$w/oos-update"
cp "$root/rootfs/out/e2fsck" "$w/e2fsck"
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
work = Path(sys.argv[2])
with (work / 'card').open('wb') as output:
    output.truncate(start + size + 512)
    output.seek(parm)
    output.write(b'PARM ' + board['OOS_PARM_MARKS'].encode()
                 + b' 0x00020000@0x0002c000(recovery) 0x00038000@0x0004c000(backup)')
    output.seek(start - 512)
    output.write(bytes([91]) * 512)
    output.seek(start + size)
    output.write(bytes([92]) * 512)
with (work / 'card').open('rb') as source:
    before = hashlib.sha256(source.read(start)).hexdigest()
(work / 'protected.sha256').write_text(before)
PY
docker run --rm --pull=never --network=none --platform linux/arm64 -v "$w:/w" "$image" sh -euc '
    mkdir /tmp/storage-tools
    ln -s /w/busybox /tmp/storage-tools/mke2fs
    ln -s /w/e2fsck /tmp/storage-tools/e2fsck
    export PATH=/tmp/storage-tools:$PATH
    /w/oos-update storage-data --card /w/card
    /w/oos-update storage-data --card /w/card
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
    assert source.read() == bytes([92]) * 512, 'write beyond OOS data'
print('storage-data: first-boot format and repeat repair passed; other areas unchanged')
PY
