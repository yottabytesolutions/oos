"""Validate assembled content, relationships and receipts before counting image evidence."""
import gzip
import hashlib
import importlib.util
import os
from pathlib import Path
import struct
import subprocess
import sys
import tarfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import output
from check_inventory import elf_static

ROOT = output.ROOT


def boot_module():
    sys.path.insert(0, str(ROOT / 'boot'))
    spec = importlib.util.spec_from_file_location('oos_bootimg', ROOT / 'boot/oos-bootimg.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def cpio(data):
    entries, offset = {}, 0
    while offset + 110 <= len(data):
        header = data[offset:offset + 110]
        if header[:6] != b'070701':
            raise ValueError('invalid newc header')
        fields = [int(header[6 + i * 8:14 + i * 8], 16) for i in range(13)]
        mode, size, length = fields[1], fields[6], fields[11]
        start = offset + 110
        if length < 1 or start + length > len(data) or data[start + length - 1] != 0:
            raise ValueError('truncated cpio filename')
        name = data[start:start + length - 1].decode().removeprefix('./')
        start = (start + length + 3) & ~3
        if start + size > len(data):
            raise ValueError('truncated cpio body')
        body = data[start:start + size]
        offset = (start + size + 3) & ~3
        if name == 'TRAILER!!!':
            if any(data[offset:]):
                raise ValueError('unexpected trailing cpio content')
            return entries
        if name in entries or name.startswith('/') or '..' in Path(name).parts:
            raise ValueError('duplicate or unsafe cpio path')
        entries[name] = (mode, body)
    raise ValueError('cpio has no trailer')


def boot_content(blob, kernel, dtb, ramdisk):
    boot = boot_module()
    fields, k, r, second = boot.parts(blob)
    if fields[7] != boot.PAGE or any(len(part) != size for part, size in zip((k, r, second), (fields[0], fields[2], fields[4]))):
        raise ValueError('invalid boot section bounds')
    if blob[boot.ID_OFFSET:boot.ID_OFFSET + 32] != boot.image_id(blob[:boot.PAGE], k, r, second):
        raise ValueError('boot image ID mismatch')
    boot.check_kernel(k)
    if k != kernel or r != ramdisk + bytes(-len(ramdisk) % 4):
        raise ValueError('boot kernel/ramdisk do not match build outputs')
    if boot.rsce_entries(second) != [('rk-kernel.dtb', dtb)] or not boot.dtb_bootargs(dtb):
        raise ValueError('boot DTB mismatch or absent bootargs')


def shipped(entries, target):
    binaries = target / 'aarch64-unknown-linux-musl/release'
    sources = {f'bin/{name}': binaries / name for name in ('oos-ui', 'oos-fpgad', 'oos-update')}
    sources['bin/oos-hw'] = target / 'hw/oos-hw'
    bundle = target / 'gpu/gpu'
    output.verify(bundle)
    for path in bundle.rglob('*'):
        if path.is_symlink():
            name = path.relative_to(bundle).as_posix()
            mode, body = entries.get(name, (0, b''))
            if mode & 0o170000 != 0o120000 or body != os.fsencode(os.readlink(path)):
                raise ValueError(f'shipped GPU link mismatch: {name}')
        if path.is_file() and not path.is_symlink():
            sources[path.relative_to(bundle).as_posix()] = path
    for name, source in sources.items():
        if name.startswith('bin/oos-') and name != 'bin/oos-gpu':
            output.verify(source)
            elf_static(source.read_bytes())
        mode, body = entries.get(name, (0, b''))
        if (mode & 0o170000 != 0o100000 or not body or body != source.read_bytes()
                or mode & 0o777 != source.stat().st_mode & 0o777):
            raise ValueError(f'shipped content mismatch: {name}')
    for name in ('init', 'etc/rcS', 'bin/busybox', 'bin/oos-gpu', 'lib/ld-linux-aarch64.so.1'):
        if name not in entries or not entries[name][1]:
            raise ValueError(f'missing shipped artifact: {name}')
    print(f'image artifacts: {len(sources)} shipped source/bundle files verified (including oos-hw and GPU)')


def kernel_checksums(base, names):
    recorded = {}
    for line in (base / 'SHA256SUMS').read_text().splitlines():
        digest, name = line.split()
        name = Path(name).name
        if name in recorded or name not in names or len(digest) != 64:
            raise ValueError('unexpected kernel checksum inventory')
        recorded[name] = digest
        if hashlib.sha256((base / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f'kernel checksum mismatch: {name}')
    if set(recorded) != names or len(names) < 2:
        raise ValueError('missing kernel/DTB checksum outputs')


def validate():
    target = output.target()
    image = target / 'image'
    kernel_base = ROOT / 'kernel/out/base'
    kernel_names = {'Image'} | {path.stem + '.dtb' for path in (ROOT / 'kernel/dts').glob('*.dts')}
    kernel_checksums(kernel_base, kernel_names)
    blob = (image / 'oos-dev.img').read_bytes()
    dtb = image / 'oos-dho900.dtb'
    ramdisk = (image / 'initramfs.cpio.gz').read_bytes()
    boot_content(blob, (ROOT / 'kernel/out/base/Image').read_bytes(), dtb.read_bytes(), ramdisk)
    entries = cpio(gzip.decompress(ramdisk))
    shipped(entries, target)
    expected = hashlib.sha256(blob).hexdigest()
    if (image / 'oos-dev.img.sha256').read_text().split() != [expected, 'oos-dev.img']:
        raise ValueError('image checksum mismatch')
    package = image / 'oos-dev.oosupd'
    gel = image / 'DHO800_DHO900_OOS_Install.GEL'
    for path in (image / 'oos-dev.img', image / 'oos-dev.build', package, gel):
        output.verify(path)
    raw = package.read_bytes()
    header, packaged = raw.split(b'\n\n', 1)
    if not header.startswith(b'OOS-UPDATE 1\n') or packaged != blob:
        raise ValueError('update package payload mismatch')
    subprocess.run(['cargo', 'run', '--offline', '--locked', '--release', '-p', 'oos-fpgad', '--bin',
                    'oos-update', '--', 'verify', str(package)], cwd=ROOT, check=True)
    with tarfile.open(gel, 'r:gz') as archive:
        if set(archive.getnames()) != {'shell', 'shell/do_extract.sh', 'oos', 'oos/oos-update', 'oos/oos.oosupd'}:
            raise ValueError('unexpected GEL inventory')
        for name, data in [('oos/oos.oosupd', raw), ('oos/oos-update', (target / 'aarch64-unknown-linux-musl/release/oos-update').read_bytes())]:
            if archive.extractfile(name).read() != data:
                raise ValueError(f'GEL payload mismatch: {name}')
        script = archive.extractfile('shell/do_extract.sh').read()
        expected_script = (ROOT / 'boot/gel/do_extract.sh').read_bytes().replace(
            b'@OOS_GEL_STEM@', boot_module().boardconf.load()['OOS_GEL_STEM'].encode())
        if script != expected_script:
            raise ValueError('invalid GEL extraction script')
        for name, mode in [('shell/do_extract.sh', 0o755), ('oos/oos-update', 0o755), ('oos/oos.oosupd', 0o644)]:
            if archive.getmember(name).mode != mode:
                raise ValueError(f'invalid GEL mode: {name}')
    print('image content, checksums, signatures and receipts verified; no hardware used')


if __name__ == '__main__':
    try:
        validate()
    except (ValueError, OSError, struct.error, tarfile.TarError, subprocess.SubprocessError) as exc:
        sys.exit(f'image validation failed: {exc}')
