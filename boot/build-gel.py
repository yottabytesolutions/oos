#!/usr/bin/env python3
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
"""build-gel.py: the update file that installs OOS from the running stock system, without opening the scope
(docs/install-usb.md).

    boot/build-gel.py        -> $CARGO_TARGET_DIR/image/DHO800_DHO900_OOS_Install.GEL

From the selected image/oos-dev.oosupd and oos-update, after boot/sign-dev-image.sh. The stock update takes any gzip tar
whose name ends in .GEL and holds DHO800_DHO900, unpacks it and runs its shell/do_extract.sh.
"""
from pathlib import Path
import sys
import tempfile

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent / "tools")]
import boardconf
from gel_package import write_gel
import output

def main():
    stem = boardconf.load()["OOS_GEL_STEM"]
    target = output.target()
    package = target / "image/oos-dev.oosupd"
    updater = target / "aarch64-unknown-linux-musl/release/oos-update"
    gel = package.with_name(stem + "_Install.GEL")
    before = output.revision()
    for path in (package, updater):
        if not path.is_file():
            sys.exit(f"missing {path}: build with boot/build-dev-image.sh, then sign with boot/sign-dev-image.sh")
        output.verify(path)
    with tempfile.TemporaryDirectory(dir=gel.parent) as temp:
        pending = Path(temp) / gel.name
        with pending.open("wb") as destination:
            write_gel(destination, package, updater.read_bytes(), stem)
        # Recheck receipts in case an input changed while archiving.
        for path in (package, updater):
            output.verify(path)
        pending.replace(gel)
    output.stamp(gel, before)
    print(f"{gel}: {gel.stat().st_size} bytes")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError) as exc:
        sys.exit(f"build-gel: {exc}")
