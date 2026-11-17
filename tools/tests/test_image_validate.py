# SPDX-License-Identifier: GPL-2.0-or-later
"""Assembled image validation uses synthetic checksums, boot sections and CPIO."""
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import image_validate


class ImageValidation(unittest.TestCase):
    def test_kernel_checksum_inventory_rejects_missing_extra_and_changed_outputs(self):
        import hashlib
        with tempfile.TemporaryDirectory() as d:
            base = Path(d)
            names = {'Image', 'fixture.dtb'}
            for name in names:
                (base / name).write_bytes(b'fixture')
            line = hashlib.sha256(b'fixture').hexdigest()
            manifest = base / 'SHA256SUMS'
            manifest.write_text(''.join(f'{line}  /out/{name}\n' for name in sorted(names)))
            image_validate.kernel_checksums(base, names)
            (base / 'fixture.dtb').write_bytes(b'corrupt')
            with self.assertRaises(ValueError):
                image_validate.kernel_checksums(base, names)
            (base / 'fixture.dtb').write_bytes(b'fixture')
            manifest.write_text(f'{line}  /out/Image\n')
            with self.assertRaises(ValueError):
                image_validate.kernel_checksums(base, names)
            manifest.write_text(f'{line}  /out/unexpected\n')
            with self.assertRaises(ValueError):
                image_validate.kernel_checksums(base, names)

    def test_boot_sections_and_cpio_have_a_positive_content_fixture(self):
        import struct
        boot = image_validate.boot_module()
        kernel = bytearray(64); kernel[0x38:0x3c] = b'ARM\x64'
        struct.pack_into('<QQ', kernel, 8, 0, len(kernel))
        tokens = (struct.pack('>I', 1) + bytes(4) + struct.pack('>I', 1) + b'chosen\0\0' +
                  struct.pack('>III', 3, 8, 0) + b'console\0' + struct.pack('>III', 2, 2, 9))
        strings = b'bootargs\0'
        dtb = struct.pack('>10I', 0xd00dfeed, 40 + len(tokens) + len(strings),
                          40, 40 + len(tokens), 0, 17, 16, 0, len(strings), len(tokens)) + tokens + strings
        ramdisk = __import__('gzip').compress(boot.cpio([('init', 0o100755, b'#!/bin/sh\n')]))
        blob = boot.build(bytes(kernel), dtb, ramdisk)
        image_validate.boot_content(blob, bytes(kernel), dtb, ramdisk)
        self.assertEqual(image_validate.cpio(__import__('gzip').decompress(ramdisk))['init'][1], b'#!/bin/sh\n')
        damaged = bytearray(blob); damaged[boot.PAGE] ^= 1
        with self.assertRaises(ValueError):
            image_validate.boot_content(bytes(damaged), bytes(kernel), dtb, ramdisk)

    def test_nonempty_broken_assembled_content_and_cpio_are_rejected(self):
        with self.assertRaises((ValueError, SystemExit)):
            image_validate.boot_content(b'nonempty broken image', b'k', b'd', b'r')
        for data in (b'nonempty', b'070701' + b'0' * 104):
            with self.assertRaises(ValueError):
                image_validate.cpio(data)


if __name__ == '__main__':
    unittest.main()
