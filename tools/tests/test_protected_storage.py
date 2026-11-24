# SPDX-License-Identifier: GPL-2.0-or-later
"""Host regressions for raw-write detection and boot-tool delegation."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import protected_storage as guard

spec = importlib.util.spec_from_file_location('boot_storage', ROOT / 'boot/storage.py')
storage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(storage)


class ProtectedStorageTests(unittest.TestCase):
    def test_rust_device_opens_and_raw_outputs_require_the_writer(self):
        for text in [
            'OpenOptions::new().write(true).open("/dev/mmcblk0")?;',
            'OpenOptions::new().write(true).open("/dev/block/by-name/uboot")?;',
            'std::fs::write("/dev/mtd0", bytes)?;',
            'libc::pwrite(fd, ptr, len, offset);',
        ]:
            self.assertTrue(guard.violations('hal/src/raw.rs', text), text)
        self.assertFalse(guard.violations(guard.WRITER, 'self.file.write_all(data)?;'))
        self.assertTrue(guard.violations('hal/src/update/card.rs', 'self.f.write_all(data)?;'))

    def test_python_legacy_partition_opens_are_rejected(self):
        for mode in ['"r+b"', '"r+b" if write else "rb"', '"wb"', '"wb", buffering=0']:
            text = f'open(target, {mode})'
            self.assertTrue(guard.violations('boot/raw.py', text), text)
        self.assertTrue(guard.violations('tools/raw.py', 'os.open("/dev/fram", os.O_RDWR)'))
        self.assertTrue(guard.violations('boot/raw.py', 'Path("/dev/mtd0").open("wb")'))
        self.assertFalse(guard.violations('boot/read.py', 'open(target, "rb")'))
        self.assertFalse(guard.violations('boot/image.py', 'open(output, "wb")'))
        self.assertFalse(guard.violations('tools/log.py', '(run / "stderr").open("w+")'))

    def test_shell_raw_writes_and_formatters_are_rejected(self):
        for text in ['dd if=image of=$dev', 'dd of="/dev/mmcblk0p4"',
                     'cat image > /dev/mtd0', 'cat image > "$dev"', 'mke2fs -L oosdata $P',
                     'e2fsck -p $P', 'tune2fs -j $P', 'flash_erase /dev/mtd0 0 0']:
            self.assertTrue(guard.violations('rootfs/initramfs/bin/raw', text), text)
        self.assertFalse(guard.violations('tools/read.sh', 'dd if=/dev/mmcblk0 bs=512 count=1'))
        self.assertFalse(guard.violations('tools/gel-sim.sh', 'dd of=/w/card bs=512'))
        self.assertTrue(guard.violations('tools/other.sh', 'dd of=/w/card bs=512'))

    def test_c_and_spi_flash_programming_are_rejected(self):
        self.assertTrue(guard.violations('tools/raw.c', 'open("/dev/mtd0", O_RDWR);'))
        self.assertTrue(guard.violations('hal/src/dho/boot/io.rs', 'open("/dev/spidev1.0"); send(b"PROG");'))
        self.assertFalse(guard.violations('hal/src/dho/boot/io.rs', 'open("/dev/spidev1.0"); send(b"BOOT");'))

    def test_boot_bridge_submits_exact_bytes_and_propagates_failure(self):
        with patch.dict('os.environ', {'OOS_UPDATE': 'checked-update'}), patch.object(storage.subprocess, 'run') as run:
            storage.write('card.img', 'boot', 62914560, b'KRNL')
            run.assert_called_once_with(['checked-update', 'storage-write', 'boot', '62914560',
                                         '--host-card', 'card.img', '--execute'], input=b'KRNL', check=True)
            run.side_effect = storage.subprocess.CalledProcessError(1, 'checked-update')
            with self.assertRaises(storage.subprocess.CalledProcessError):
                storage.write('card.img', 'misc', 0, b'bad')

    def test_legacy_bcb_execution_refuses_before_any_writable_open(self):
        sys.path.insert(0, str(ROOT / 'boot'))
        spec = importlib.util.spec_from_file_location('bcb', ROOT / 'boot/bcb.py')
        bcb = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(bcb)
        import io
        from unittest.mock import mock_open
        reader = mock_open(read_data=bytes(512))
        with patch.object(sys, 'argv', ['bcb.py', 'set', 'card.img', 'recovery', '--execute']), \
                patch('builtins.open', reader), patch('sys.stdout', new_callable=io.StringIO):
            with self.assertRaisesRegex(SystemExit, 'protected by ADR 0011'):
                bcb.main()
        reader.assert_called_once_with('card.img', 'rb', buffering=0)
