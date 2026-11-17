# SPDX-License-Identifier: GPL-2.0-or-later
"""Release sidecars preserve the signed bundle and expose its statement as JSONL."""
import base64
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "ci/attestation_assets.py"


class AttestationAssetsTests(unittest.TestCase):
    def attach(self, bundle):
        work = tempfile.TemporaryDirectory()
        self.addCleanup(work.cleanup)
        root = Path(work.name)
        source = root / "bundle.json"
        source.write_text(json.dumps(bundle, indent=2), encoding="utf-8")
        assets = [root / name for name in ("oos-v0.2.0.img", "SHA256SUMS", "linux-base.tar.xz")]
        for asset in assets:
            asset.write_bytes(b"original artifact\n")
        result = subprocess.run([sys.executable, "-I", str(SCRIPT), str(source), *map(str, assets)],
                                capture_output=True, text=True, check=False)
        return source, assets, result

    def test_bundle_and_statement_for_every_asset(self):
        statement = {"_type": "https://in-toto.io/Statement/v1", "subject": [{"name": "oos-v0.2.0.img"}]}
        payload = base64.b64encode(json.dumps(statement, indent=2).encode()).decode()
        source, assets, result = self.attach({"dsseEnvelope": {"payload": payload, "signatures": []}})
        self.assertEqual(result.returncode, 0, result.stderr)
        for asset in assets:
            self.assertEqual(asset.read_bytes(), b"original artifact\n")
            self.assertEqual(Path(str(asset) + ".sigstore.json").read_bytes(), source.read_bytes())
            lines = Path(str(asset) + ".intoto.jsonl").read_text().splitlines()
            self.assertEqual(len(lines), 1)
            self.assertEqual(json.loads(lines[0]), statement)

    def test_bundle_without_exposed_statement(self):
        source, assets, result = self.attach({"messageSignature": {}})
        self.assertEqual(result.returncode, 0, result.stderr)
        for asset in assets:
            self.assertEqual(Path(str(asset) + ".sigstore.json").read_bytes(), source.read_bytes())
            self.assertFalse(Path(str(asset) + ".intoto.jsonl").exists())

    def test_invalid_exposed_statement_fails_before_writing_sidecars(self):
        _, assets, result = self.attach({"dsseEnvelope": {"payload": "not base64"}})
        self.assertNotEqual(result.returncode, 0)
        for asset in assets:
            self.assertFalse(Path(str(asset) + ".sigstore.json").exists())


if __name__ == "__main__":
    unittest.main()
