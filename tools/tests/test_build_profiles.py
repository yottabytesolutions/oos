"""Disposable profile execution uses only selected roles and never prepares images."""
import contextlib
import importlib.util
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import build_profiles as profiles


class Profiles(unittest.TestCase):
    def test_gpu_runs_daemon_platform_software_egl_probe(self):
        calls = []
        fake = lambda *a, **k: calls.append((a, k)) or ('x86_64' if a[:2] == ('info', '--format') else '')
        with patch.object(profiles.build_env, 'resolve', return_value='sha256:' + '1' * 64) as resolve:
            with patch.object(profiles.build_env, 'docker', fake), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(profiles.inventory('gpu'), [])
        resolve.assert_called_once_with('gpu-test')
        args, kwargs = calls[-1]
        self.assertIn('--pull=never', args)
        self.assertIn('--network=none', args)
        self.assertEqual(args[args.index('--platform') + 1], 'linux/amd64')
        self.assertIn('LIBGL_ALWAYS_SOFTWARE=1', args)
        self.assertIn('eglMakeCurrent', kwargs['input'])
        self.assertFalse(any(a in ('build', 'pull', '--privileged') for args, _ in calls for a in args))

    def test_missing_emulation_is_a_selected_failure(self):
        def docker(*args, **kwargs):
            if args[0] == 'run':
                raise ValueError('exec format error')
            return 'aarch64' if args[:2] == ('info', '--format') else ''
        with patch.object(profiles.build_env, 'resolve', return_value='sha256:' + '1' * 64):
            with patch.object(profiles.build_env, 'docker', docker):
                errors = profiles.inventory('gpu')
        self.assertEqual(len(errors), 1)
        self.assertIn('gpu-test', errors[0])

    def test_sim_checks_net_admin_nft_ipv6_and_ssh_utilities(self):
        calls = []
        with patch.object(profiles.build_env, 'docker', lambda *a, **k: calls.append(a) or ''):
            with patch.object(profiles.build_env, 'resolve', return_value='sha256:' + '1' * 64), patch.object(profiles, 'qemu'), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(profiles.inventory('sim'), [])
        probes = [a for a in calls if '--cap-add' in a]
        self.assertEqual(len(probes), 2)
        for args in probes:
            self.assertIn('NET_ADMIN', args)
            self.assertIn('net.ipv6.conf.all.disable_ipv6=0', args)
            self.assertIn('nft add table', args[-1])
            self.assertIn('ip -6', args[-1])
        self.assertTrue(any('command -v sshpass' in a[-1] for a in probes))

    def test_offline_daemon_missing_images_and_source_all_reported(self):
        with patch.object(profiles.build_env, 'docker', side_effect=ValueError('offline')):
            with patch.object(profiles.build_env, 'resolve', side_effect=ValueError('missing')):
                with patch.object(profiles, 'kernel_source', side_effect=ValueError('checksum')):
                    with patch.object(profiles, 'qemu', side_effect=OSError('missing')):
                        errors = profiles.inventory('image')
        self.assertTrue(any('docker:' in e for e in errors))
        self.assertTrue(any('kernel-source' in e for e in errors))
        self.assertFalse(any('qemu' in e for e in errors))
        for role in ('kernel', 'rootfs-nft', 'sim-usb', 'sim-wlan', 'gpu-build', 'initramfs'):
            self.assertTrue(any(e.startswith(role + ':') for e in errors))

    def test_qemu_is_required_only_by_sim_profile(self):
        for profile in ('gpu', 'image', 'sim'):
            with patch.object(profiles.build_env, 'docker', return_value='aarch64'), \
                    patch.object(profiles.build_env, 'resolve', return_value='sha256:' + '1' * 64), \
                    patch.object(profiles, 'kernel_source'), \
                    patch.object(profiles, 'qemu', side_effect=OSError('missing')) as qemu, \
                    contextlib.redirect_stdout(io.StringIO()):
                errors = profiles.inventory(profile)
            self.assertEqual(qemu.call_count, int(profile == 'sim'))
            self.assertEqual(any(e.startswith('qemu:') for e in errors), profile == 'sim')


if __name__ == '__main__':
    unittest.main()
