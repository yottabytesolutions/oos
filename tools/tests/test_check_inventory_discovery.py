# SPDX-License-Identifier: GPL-2.0-or-later
"""Discovery and artifact contracts, independent of Docker or private fixtures."""
import importlib.util
import io
import json
from contextlib import redirect_stdout
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import check_inventory as host
import image_validate
import image_suite
import script_syntax


class Discovery(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.write('valid.py', 'x = 1\n')
        self.write('valid.sh', '#!/bin/sh\nx=1\n')

    def write(self, name, text, mode=0o600):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text); path.chmod(mode)
        return path

    def test_nested_invalid_python_and_rootfs_shell_fail(self):
        path = self.write('tools/nested/a space.py', 'if broken\n')
        with self.assertRaises(Exception):
            script_syntax.check(self.root, self.root / 'out')
        path.unlink()
        self.write('rootfs/initramfs/bin/extensionless', '#!/bin/sh\nif then\n', 0o700)
        with self.assertRaises(subprocess.CalledProcessError):
            script_syntax.check(self.root, self.root / 'out')

    def test_syntax_does_not_execute_and_keeps_bytecode_under_target(self):
        self.write('a space/danger.py', 'raise RuntimeError("never execute")\n')
        self.write('rootfs/entry', '#!/usr/bin/env bash\n[[ x == x ]]\n', 0o700)
        self.write('.githooks/hook', '#!/bin/sh\nexit 7\n', 0o700)
        script_syntax.check(self.root, self.root / 'out')
        self.assertTrue((self.root / 'out/script-bytecode/a space/danger.pyc').exists())
        self.assertFalse((self.root / 'a space/__pycache__').exists())

    def test_unknown_executable_and_unmapped_fragment_fail(self):
        for name, text in [('tools/new', 'echo unchecked\n'), ('tools/new.sh', 'x=1\n'),
                           ('tools/new', '#!/usr/bin/env perl\n1;\n'),
                           ('tools/new', '#!/usr/bin/env python3-unclassified\nx=1\n'),
                           ('tools/new', '#!/system/bin/shutdown\nx=1\n'),
                           ('tools/new', '#!/bin/busybox shutdown\nx=1\n')]:
            path = self.write(name, text, 0o700)
            with self.assertRaises(ValueError):
                script_syntax.interpreter(path, name)
            path.unlink()
        for name, text, shell in [('tools/output.sh', 'x=1\n', 'sh'),
                                  ('boot/gel/do_extract.sh', '#!/system/bin/sh\n', 'mksh'),
                                  ('tools/new', '#!/usr/bin/env -S bash -e\n', 'bash')]:
            path = self.write(name, text)
            self.assertEqual(script_syntax.interpreter(path, name), shell)

    def test_empty_git_and_empty_test_selection_fail(self):
        (self.root / 'valid.py').unlink(); (self.root / 'valid.sh').unlink()
        with self.assertRaises(ValueError):
            script_syntax.check(self.root, self.root / 'out')
        tests = self.root / 'tests'; tests.mkdir()
        with self.assertRaises(ValueError):
            host.nonempty_tests(tests)
        for text in ('Ran 0 tests in 0s', 'PASS', 'Ran 2 tests in 0s\nOK (skipped=2)'):
            with self.assertRaises(ValueError):
                host.test_summary(text, 2)
        with redirect_stdout(io.StringIO()) as log:
            host.test_summary("test_fixture (fixture.Tests) ... skipped 'requires fixture'\n"
                              'Ran 2 tests in 0.1s\nOK (skipped=1)', 2)
        self.assertIn("skip reason: 'requires fixture'", log.getvalue())
        with self.assertRaisesRegex(ValueError, 'missing reasons'):
            host.test_summary('Ran 2 tests in 0.1s\nOK (skipped=1)', 2)

    def test_git_tests_outside_discovered_roots_fail(self):
        self.write('ui/tools/test_fixture.py', 'pass\n')
        self.write('tools/tests/test_fixture.py', 'pass\n')
        self.write('tools/privacy/test_fixture.py', 'pass\n')
        host.validate_test_roots(self.root)
        for name in ('test_new.py', 'nested/test_new.py', 'tools/test_new.py'):
            path = self.write(name, 'pass\n')
            self.assertRaisesRegex(ValueError, name, host.validate_test_roots, self.root)
            path.unlink()


class Artifacts(unittest.TestCase):
    def test_arm_requires_every_binary_and_verified_receipts(self):
        with tempfile.TemporaryDirectory() as d:
            target = Path(d)
            release = target / 'aarch64-unknown-linux-musl/release'; release.mkdir(parents=True)
            for name in ('oos-ui', 'oos-fpgad'):
                (release / name).write_bytes(b'fixture')
            seen = []
            with patch.object(host.output, 'target', return_value=target), patch.object(host, 'run') as run, \
                    patch.object(host.output, 'verify', side_effect=lambda p: seen.append(p)), \
                    patch.object(host, 'elf_static'):
                with self.assertRaises(FileNotFoundError):
                    host.arm()
                self.assertEqual(seen[-1].name, 'oos-update')
                self.assertIn('--bins', run.call_args.args[0])
            with patch.object(host, 'run'), patch.object(host.output, 'verify', side_effect=ValueError('stale receipt')):
                with self.assertRaisesRegex(ValueError, 'stale'):
                    host.arm()

    def test_elf_headers_reject_wrong_architecture_dynamic_and_empty(self):
        import struct
        data = bytearray(120)
        data[:7] = b'\x7fELF\x02\x01\x01'
        struct.pack_into('<HH', data, 16, 2, 183)
        struct.pack_into('<Q', data, 24, 0x10000)
        struct.pack_into('<Q', data, 32, 64)
        struct.pack_into('<HH', data, 54, 56, 1)
        struct.pack_into('<IIQQQQQQ', data, 64, 1, 5, 0, 0x10000, 0, len(data), len(data), 0x1000)
        host.elf_static(data)
        for offset, value in [(16, 0), (18, 62), (64, 3), (64, 2), (54, 1)]:
            broken = bytearray(data); broken[offset] = value
            with self.assertRaises(ValueError):
                host.elf_static(broken)
        with self.assertRaises(ValueError):
            host.elf_static(b'not an executable')

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

    def test_simulation_preparation_order_and_failure_propagation(self):
        seen = []
        def run(args):
            seen.append(args)
            if args[-1] == 'tools/ssh-sim.sh':
                raise subprocess.CalledProcessError(7, args)
        with patch.object(host, 'run', side_effect=run):
            with self.assertRaises(subprocess.CalledProcessError):
                host.prepare_sim(True)
        self.assertEqual([c[-1] for c in seen], ['initramfs', 'rootfs-nft', 'rootfs-dropbear',
                         'rootfs-sftp-server', 'rootfs/build-nft.sh',
                         'rootfs/build-dropbear.sh', 'rootfs/build-sftp-server.sh', 'tools/ssh-sim.sh'])

    def test_simulations_prepare_only_required_existing_roles(self):
        roles = json.loads((ROOT / 'tools/container-images.json').read_text())['roles']
        for ssh, builders, script in ((True, ('nft', 'dropbear', 'sftp-server'), 'ssh-sim'),
                                      (False, ('nft',), 'fw-sim')):
            with self.subTest(ssh=ssh):
                seen = []
                def run(args):
                    if args[:3] == ['sh', 'tools/build-env.sh', 'prepare']:
                        self.assertIn(args[3], roles, 'simulation requests a removed environment role')
                    seen.append(args)
                with patch.object(host, 'run', side_effect=run):
                    host.prepare_sim(ssh)
                expected = [['sh', 'tools/build-env.sh', 'prepare', role]
                            for role in ('initramfs', *(f'rootfs-{name}' for name in builders))]
                expected += [['sh', f'rootfs/build-{name}.sh'] for name in builders]
                expected += [['sh', f'tools/{script}.sh']]
                self.assertEqual(seen, expected)


class ImageTier(unittest.TestCase):
    def test_suite_requires_kernel_and_validated_outputs_before_simulations(self):
        with tempfile.TemporaryDirectory() as d:
            target = Path(d)
            (target / 'image').mkdir()
            seen = []
            with patch.object(image_suite.output, 'target', return_value=target), \
                    patch.object(image_suite, 'kernel_source', side_effect=lambda: seen.append('kernel-source')), \
                    patch.object(image_suite, 'run', side_effect=lambda c: seen.append(c)), \
                    patch.object(image_suite, 'validate', side_effect=lambda: seen.append('validate')), \
                    patch.object(image_suite.subprocess, 'run') as manifest, patch.dict(os.environ):
                image_suite.main()
            self.assertEqual(seen[0], 'kernel-source')
            self.assertIn(['cargo', 'xtask', 'doctor', '--image'], seen)
            validation = seen.index('validate')
            for command in (['sh', 'kernel/build-base.sh'], ['sh', 'boot/build-dev-image.sh'],
                            ['sh', 'boot/sign-dev-image.sh', '--community'], ['python3', '-I', 'boot/build-gel.py']):
                self.assertLess(seen.index(command), validation)
            self.assertEqual(seen[validation + 1:], [['sh', f'tools/{name}.sh'] for name in
                             ('unit-dtb-check', 'gel-sim', 'usb-sim', 'wlan-sim')])
            self.assertTrue(manifest.call_args.kwargs['check'])

    def test_missing_kernel_source_cannot_pass_or_launch_builders(self):
        with patch.dict(os.environ), patch.object(image_suite, 'kernel_source', side_effect=ValueError('missing source')), \
                patch.object(image_suite, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'missing source'):
                image_suite.main()
            run.assert_not_called()

    def test_failed_image_command_stops_validation_and_simulations(self):
        with patch.dict(os.environ), patch.object(image_suite, 'kernel_source'), \
                patch.object(image_suite, 'run', side_effect=subprocess.CalledProcessError(7, ['fixture'])), \
                patch.object(image_suite, 'validate') as validate:
            with self.assertRaises(subprocess.CalledProcessError):
                image_suite.main()
            validate.assert_not_called()


if __name__ == '__main__':
    unittest.main()
