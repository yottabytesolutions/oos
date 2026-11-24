# SPDX-License-Identifier: GPL-2.0-or-later
"""Write the stock update archive from a signed package and its target updater."""
import gzip
import io
import os
from pathlib import Path
import tarfile

SCRIPT = Path(__file__).resolve().parent / "gel/do_extract.sh"


def _member(archive, name, source, size, mode):
    member = tarfile.TarInfo(name)
    member.size, member.mode = size, mode
    archive.addfile(member, source)


def write_gel(destination, package, updater, stem):
    """Write a GEL to an open binary file. All archive metadata is fixed."""
    script = SCRIPT.read_bytes().replace(b"@OOS_GEL_STEM@", stem.encode("ascii"))
    with gzip.GzipFile(filename="", fileobj=destination, mode="wb", mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w|", format=tarfile.GNU_FORMAT) as archive:
            for name in ("shell", "oos"):
                member = tarfile.TarInfo(name)
                member.type, member.mode = tarfile.DIRTYPE, 0o755
                archive.addfile(member)
            _member(archive, "shell/do_extract.sh", io.BytesIO(script), len(script), 0o755)
            _member(archive, "oos/oos-update", io.BytesIO(updater), len(updater), 0o755)
            with package.open("rb") as source:
                _member(archive, "oos/oos.oosupd", source, os.fstat(source.fileno()).st_size, 0o644)
