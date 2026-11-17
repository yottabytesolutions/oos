"""tools/frontpanel.py: how each system sets 1.5 Mbaud."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("frontpanel", ROOT / "tools/frontpanel.py")
frontpanel = importlib.util.module_from_spec(spec)
spec.loader.exec_module(frontpanel)


class SpeedPlan(unittest.TestCase):
    def test_linux_takes_its_termios_constant(self):
        self.assertEqual(frontpanel.speed_plan("Linux", SimpleNamespace(B1500000=0o010012)), ("termios", 0o010012))

    def test_macos_sets_the_rate_by_ioctl(self):
        self.assertEqual(frontpanel.speed_plan("Darwin", SimpleNamespace()), ("ioctl", 0x80085402))

    def test_the_bsds_take_the_rate_itself(self):
        for system in ("FreeBSD", "OpenBSD", "NetBSD", "DragonFly"):
            with self.subTest(system=system):
                self.assertEqual(frontpanel.speed_plan(system, SimpleNamespace()), ("termios", 1500000))

    def test_an_unknown_system_fails_clearly_without_the_linux_value(self):
        with self.assertRaises(SystemExit) as raised:
            frontpanel.speed_plan("SunOS", SimpleNamespace())
        self.assertIn("1500000 baud on SunOS", str(raised.exception))

    def test_selftest_still_runs_anywhere(self):
        self.assertEqual(frontpanel.frame(1, 0).hex(), "fa0100" + f"{frontpanel.crc7(1, 0):02x}" + "af")


if __name__ == "__main__":
    unittest.main()
