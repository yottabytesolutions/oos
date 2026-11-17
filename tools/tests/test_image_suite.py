# SPDX-License-Identifier: GPL-2.0-or-later
"""Image suite ordering and failures are checked without running builders."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import image_suite


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
                    patch.object(image_suite.subprocess, 'run') as manifest, \
                    patch.dict(os.environ, {'OOS_SIGNING_KEY': '/path/to/release.key'}):
                image_suite.main()
                self.assertEqual(os.environ['OOS_SIGNING_KEY'], '/path/to/release.key')
            self.assertEqual(seen[0], 'kernel-source')
            self.assertIn(['cargo', 'xtask', 'doctor', '--image'], seen)
            validation = seen.index('validate')
            for command in (['sh', 'kernel/build-base.sh'], ['sh', 'boot/build-dev-image.sh'],
                            ['sh', 'boot/sign-dev-image.sh'], ['python3', '-I', 'boot/build-gel.py']):
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
