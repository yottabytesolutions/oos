"""tools/ci/host_scripts.py: which scripts count as host scripts, and that shellcheck findings fail."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("host_scripts", ROOT / "tools/ci/host_scripts.py")
host_scripts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host_scripts)


class Selection(unittest.TestCase):
    def setUp(self):
        self.names = host_scripts.host_scripts(ROOT)

    def test_host_scripts_and_fragments_are_in(self):
        for name in ('tools/sdcard-clone.sh', 'tools/lib/portable.sh', 'tools/output.sh', 'ui/tools/layout-check.sh',
                     '.githooks/pre-commit'):
            self.assertIn(name, self.names)

    def test_scope_and_container_scripts_are_out(self):
        for name in ('rootfs/initramfs/init', 'rootfs/initramfs/bin/oos-watchdog', 'boot/gel/do_extract.sh',
                     'tools/platform-probe/probe.sh', 'kernel/tools/git', 'boot/unit-hook-dtb.sh'):
            self.assertNotIn(name, self.names)
        self.assertFalse([n for n in self.names if n.startswith(host_scripts.SCOPE)])

    def test_python_is_out(self):
        self.assertFalse([n for n in self.names if n.endswith('.py')])


@unittest.skipIf(shutil.which('shellcheck') is None, 'shellcheck is not installed')
class Check(unittest.TestCase):
    def run_on(self, text):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / 'a.sh').write_text(text)
            host_scripts.check(Path(tmp), ['a.sh'])

    def test_clean_posix_sh_passes(self):
        self.run_on('#!/bin/sh\nset -eu\nx=$(pwd)\necho "$x"\n')

    def test_a_bashism_fails(self):
        with self.assertRaises(ValueError):
            self.run_on('#!/bin/sh\n[[ -n "$1" ]] && echo yes\n')


if __name__ == '__main__':
    unittest.main()
