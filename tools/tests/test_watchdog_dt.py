"""The kexec watchdog must survive dw_wdt_stop: its DT node must have no reset line."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


def check_watchdog(text):
    # Check board definitions and overrides, including path references. The pinned rk3399.dtsi defines
    # watchdog@ff848000 without resets; board/UHS overrides must preserve that load-bearing invariant.
    text = re.sub(r'/\*.*?\*/|//[^\n]*', '', text, flags=re.S)
    header = r'(?:&wdt\b|(?:\w+\s*:\s*)?watchdog@ff848000\b|&\{/watchdog@ff848000\})\s*\{'
    for node in re.finditer(header, text):
        depth = 1
        end = node.end()
        while depth and end < len(text):
            depth += (text[end] == '{') - (text[end] == '}')
            end += 1
        if depth:
            raise ValueError('unterminated watchdog node')
        if re.search(r'(?:^|;)\s*resets\s*(?:=|;)', text[node.end():end - 1]):
            raise ValueError('watchdog resets would stop the counter during kexec')


class WatchdogDT(unittest.TestCase):
    def test_board_and_unit_dts_preserve_the_running_watchdog(self):
        paths = sorted((ROOT / 'kernel/dts').glob('*.dts*'))
        self.assertTrue(paths)
        for path in paths:
            with self.subTest(path=path.name):
                check_watchdog(path.read_text())

    def test_watchdog_reset_overrides_are_rejected(self):
        for node in ('&wdt', 'wdt: watchdog@ff848000', 'watchdog@ff848000', '&{/watchdog@ff848000}'):
            for prop in ('resets = <&cru 1>;', 'status = "okay"; resets = <&cru 1>;', 'resets;'):
                with self.subTest(node=node, prop=prop), self.assertRaisesRegex(ValueError, 'stop the counter'):
                    check_watchdog(node + ' { ' + prop + ' };')
        # The UHS tree inherits the board: appending an override there must fail as well.
        uhs = (ROOT / 'kernel/dts/rk3399-rigol-dho900-uhs.dts').read_text()
        with self.assertRaisesRegex(ValueError, 'stop the counter'):
            check_watchdog(uhs + '\n&wdt { resets = <&cru 1>; };\n')

    def test_comments_and_other_devices_do_not_change_the_watchdog(self):
        check_watchdog('/* &wdt { resets = <1>; }; */\n&wdt { status = "okay"; };')
        check_watchdog('&uart0 { resets = <1>; }; // &wdt { resets = <1>; };')


if __name__ == '__main__':
    unittest.main()
