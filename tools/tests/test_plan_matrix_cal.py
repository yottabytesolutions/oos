# SPDX-License-Identifier: GPL-2.0-or-later
"""Plan-matrix calibration and deployment use the shared host inputs."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zlib

ROOT = Path(__file__).resolve().parents[2]


class PlanMatrixDeployment(unittest.TestCase):
    def test_shared_deployment_includes_every_case_beside_the_script(self):
        cases = (ROOT / "tools/plan-matrix-cases.txt").read_text().splitlines()
        expected = "".join(f"### {case}\nplan {case}\n" for case in cases)
        with tempfile.TemporaryDirectory() as tmp:
            deployed = Path(tmp) / "deployed"
            deployed.mkdir()
            planner = Path(tmp) / "planner"
            planner.write_text('#!/bin/sh\nprintf "%s\\n" "$*"\n')
            planner.chmod(0o700)
            uploads = [line for line in (ROOT / "tools/device-deploy.sh").read_text().splitlines()
                       if line.startswith("send ")]
            # Exercise the shared upload wiring with local copies instead of SSH.
            harness = ('set -eu\nhere=$1\ndeployed=$2\nui=$3\nfpgad=$3\n'
                       'send() { cp "$1" "$deployed/$2"; }\n' + "\n".join(uploads))
            subprocess.run(["sh", "-s", "--", str(ROOT), str(deployed), str(planner)],
                           input=harness, text=True, check=True)
            result = subprocess.run(["sh", str(deployed / "plan-matrix.sh"),
                                     str(deployed / "oos-fpgad.new")], cwd=tmp,
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, expected)


class PlanMatrixCalibration(unittest.TestCase):
    def test_output_matches_stored_fixtures_and_vertical_reference(self):
        fixtures = ROOT / "hal/testdata/synthetic"
        stored = {p.name: p.read_bytes() for p in fixtures.glob("*.hex")}
        references = {p: p.read_bytes() for p in
                      (fixtures.parent / "adc_expected.txt", fixtures.parent / "afe_expected.txt")}
        with tempfile.TemporaryDirectory(prefix="plan-matrix-cal-") as tmp:
            target = Path(tmp) / "cal"
            command = [sys.executable, "-I", str(ROOT / "tools/generate-test-cal.py"), "--output", str(target)]
            subprocess.run(command, cwd=tmp, check=True)
            generated = {p.name: p.read_bytes() for p in target.iterdir()}
            self.assertEqual(set(generated), set(stored) | {"cal_vertical.hex"})
            for name, data in stored.items():
                self.assertEqual(generated[name], data, name)
            vertical = generated["cal_vertical.hex"]
            self.assertEqual(len(vertical), 717724)
            self.assertEqual(zlib.crc32(vertical), 0x9dce7aed)
            subprocess.run(command, cwd=tmp, check=True)
            self.assertEqual({p.name: p.read_bytes() for p in target.iterdir()}, generated)
        self.assertEqual({p.name: p.read_bytes() for p in fixtures.glob("*.hex")}, stored)
        self.assertEqual({p: p.read_bytes() for p in references}, references)


if __name__ == "__main__":
    unittest.main()
