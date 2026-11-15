#!/usr/bin/env python3
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
"""build-gel.py: the update file that installs OOS from the running stock system, without opening the scope
(docs/install-usb.md).

    boot/build-gel.py        -> $CARGO_TARGET_DIR/image/DHO800_DHO900_OOS_Install.GEL

From the selected image/oos-dev.oosupd and oos-update, after boot/sign-dev-image.sh. The stock update takes any gzip tar
whose name ends in .GEL and holds DHO800_DHO900, unpacks it and runs its shell/do_extract.sh.
"""
import gzip, io, os, sys, tarfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import boardconf

ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "tools"))
import output

BOARD = boardconf.load()
TARGET = output.target()
PKG = TARGET / "image/oos-dev.oosupd"
TOOL = TARGET / "aarch64-unknown-linux-musl/release/oos-update"
OUT = TARGET / "image" / (BOARD["OOS_GEL_STEM"] + "_Install.GEL")


def add(t, name, path, mode, stem=False):
    data = open(path, "rb").read()
    if stem:
        data = data.replace(b"@OOS_GEL_STEM@", BOARD["OOS_GEL_STEM"].encode())
    i = tarfile.TarInfo(name)
    i.size, i.mode, i.mtime = len(data), mode, int(os.path.getmtime(PKG))
    t.addfile(i, io.BytesIO(data))


def folder(t, name):
    i = tarfile.TarInfo(name)
    i.type, i.mode, i.mtime = tarfile.DIRTYPE, 0o755, int(os.path.getmtime(PKG))
    t.addfile(i)


def main():
    before = output.revision()
    for p in (PKG, TOOL):
        if not os.path.exists(p):
            sys.exit(f"missing {p}: build with boot/build-dev-image.sh, then sign with boot/sign-dev-image.sh")
        output.verify(p)
    b = io.BytesIO()
    with gzip.GzipFile(fileobj=b, mode="wb", mtime=0) as z:
        with tarfile.open(fileobj=z, mode="w", format=tarfile.GNU_FORMAT) as t:
            folder(t, "shell")
            add(t, "shell/do_extract.sh", os.path.join(HERE, "gel", "do_extract.sh"), 0o755, stem=True)
            folder(t, "oos")
            add(t, "oos/oos-update", TOOL, 0o755)
            add(t, "oos/oos.oosupd", PKG, 0o644)
    # Recheck inputs before publishing, including replacements during archive creation.
    for p in (PKG, TOOL):
        output.verify(p)
    pending = OUT.with_suffix(f".{os.getpid()}.pending")
    pending.write_bytes(b.getvalue())
    pending.replace(OUT)
    output.stamp(OUT, before)
    print(f"{OUT}: {len(b.getvalue())} bytes")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError) as exc:
        sys.exit(f"build-gel: {exc}")
