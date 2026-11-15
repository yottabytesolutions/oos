"""Review regressions for clean Docker hosts and scoped native-tool receipts."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import build_env
import output


class CleanDocker(unittest.TestCase):
    def test_prepare_pulls_base_before_buildkit_build_and_local_inspection(self):
        contract, tag, key = build_env.identity('gpu-test', 'linux/arm64')
        calls = []
        local = {}
        base = dict(Os='linux', Architecture='arm64', RootFS={'Layers': ['base']},
                    RepoDigests=[contract['base']])
        derived = dict(base, Id='sha256:' + '2' * 64, Config={'Labels': {build_env.LABEL: key}},
                       RootFS={'Layers': ['base', 'derived']})
        def docker(*args, **kwargs):
            calls.append(args)
            if args[0] == 'pull':
                self.assertEqual(args, ('pull', '--platform', 'linux/arm64', contract['base']))
                local[contract['base']] = base
            elif args[0] == 'build':
                # BuildKit loads its result, but does not load FROM into the local image store.
                self.assertIn(contract['base'], local)
                local[tag] = derived
            elif args[:2] == ('image', 'inspect'):
                if args[2] not in local:
                    raise build_env.Prerequisite('missing local image')
                return json.dumps([local[args[2]]])
            return ''
        with patch.object(build_env, 'docker', docker):
            self.assertEqual(build_env.prepare('gpu-test', 'linux/arm64'), derived['Id'])
        self.assertEqual([c[0] for c in calls], ['image', 'pull', 'build', 'image', 'image'])

    def test_compiler_probes_use_repository_cwd(self):
        with patch.object(output.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, b'version')) as run:
            output.compiler_identity()
        self.assertEqual([c.kwargs.get('cwd') for c in run.call_args_list], [output.ROOT, output.ROOT])

    def test_per_tool_package_requests_and_linux_scripts_match_original_needs(self):
        expected = {'fsck': ['build-base', 'linux-headers', 'pkgconf'],
                    'kexec': ['build-base', 'linux-headers'],
                    'wlan-tools': ['build-base', 'linux-headers', 'pkgconf', 'libnl3-dev', 'libnl3-static'],
                    'nft': ['build-base', 'linux-headers', 'pkgconf', 'bison', 'flex', 'xz'],
                    'dropbear': ['build-base', 'linux-headers'],
                    'sftp-server': ['build-base', 'linux-headers']}
        lock = build_env.load()
        self.assertNotIn('rootfs', lock['roles'])
        for tool, packages in expected.items():
            self.assertEqual(lock['roles']['rootfs-' + tool]['requested'], packages)
            self.assertIn('prepare rootfs-' + tool, (ROOT / f'rootfs/build-{tool}.sh').read_text())
        for role in ('sim-usb', 'sim-wlan'):
            lines = (ROOT / f'tools/build-env/{role}.Dockerfile').read_text().splitlines()
            self.assertEqual(lines[1:], ['RUN apk add --no-cache @PACKAGES@',
                                         'RUN apk add --no-cache --no-scripts linux-lts'])
            self.assertNotIn('linux-lts', lock['roles'][role]['requested'])

    def test_image_checks_daemon_before_querying_environment(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            for name in ('boot/build-dev-image.sh', 'tools/output.sh', 'tools/output.py',
                         'rootfs/board/dho/etc/oos-board'):
                dest = root / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(ROOT / name, dest)
            kernel = root / 'kernel/out/base/Image'
            kernel.parent.mkdir(parents=True)
            kernel.touch()
            log = root / 'commands'
            fake = root / 'bin/docker'
            fake.parent.mkdir()
            fake.write_text('#!/bin/sh\nprintf "docker %s\\n" "$*" >> "$OOS_TEST_LOG"\nexit 1\n')
            fake.chmod(0o700)
            git = fake.parent / 'git'
            git.write_text('#!/bin/sh\nexit 0\n')
            git.chmod(0o700)
            resolver = root / 'tools/build-env.sh'
            resolver.write_text('#!/bin/sh\necho environment >> "$OOS_TEST_LOG"\nexit 1\n')
            resolver.chmod(0o700)
            env = {**os.environ, 'PATH': str(fake.parent) + os.pathsep + os.environ['PATH'],
                   'OOS_TEST_LOG': str(log)}
            env.pop('CARGO_TARGET_DIR', None)
            result = subprocess.run(['sh', str(root / 'boot/build-dev-image.sh')],
                                    cwd=root, env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Docker is not running', result.stderr)
            self.assertEqual(log.read_text().splitlines(), ['docker info'])


class RootfsReceipts(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        for name in ['tools/build_env.py', 'tools/build-env.sh', 'tools/container-images.json',
                     'rootfs/deps.sh', 'rootfs/deps.lock']:
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, dest)
        (self.root / 'tools/build-env.sh').chmod(0o700)
        shutil.copytree(ROOT / 'tools/build-env', self.root / 'tools/build-env')
        for script in ROOT.glob('rootfs/build-*.sh'):
            shutil.copyfile(script, self.root / 'rootfs' / script.name)
        self.patch = patch.object(output, 'ROOT', self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)

    def test_rust_changes_preserve_all_rootfs_tools_and_real_inputs_invalidate(self):
        artifacts = ('e2fsck', 'kexec', 'wpa_supplicant', 'wpa_cli', 'nft', 'dropbearmulti', 'sftp-server')
        out = self.root / 'rootfs/out'
        out.mkdir()
        for name in artifacts:
            binary = out / name
            binary.write_bytes(b'executable')
            # revision() must never be used for these native-tool receipts.
            with patch.object(output, 'revision', side_effect=AssertionError('whole tree used')):
                before = output.source_identity(binary)
                output.stamp(binary, before)
                (self.root / 'source.rs').write_text('unrelated Rust edit')
                output.verify(binary)
        cases = [('build-fsck.sh', 'e2fsck'), ('build-kexec.sh', 'kexec'),
                 ('build-wlan-tools.sh', 'wpa_supplicant'), ('deps.sh', 'nft'),
                 ('deps.lock', 'dropbearmulti'), ('build-sftp-server.sh', 'sftp-server')]
        for filename, artifact in cases:
            path = self.root / 'rootfs' / filename
            old = path.read_bytes()
            path.write_bytes(old.replace(b'2026.94', b'2026.95', 1) if filename == 'deps.lock' else old + b'\n# changed input\n')
            with self.assertRaisesRegex(ValueError, 'stale'):
                output.verify(out / artifact)
            path.write_bytes(old)
        lock = self.root / 'tools/container-images.json'
        data = json.loads(lock.read_text())
        for role in data['roles'].values():
            role['requested'].append('changed-package')
        lock.write_text(json.dumps(data))
        for name in artifacts:
            with self.assertRaisesRegex(ValueError, 'stale'):
                output.verify(out / name)

    def test_all_native_receipts_use_the_shell_environment_seam(self):
        resolver = self.root / 'tools/build-env.sh'
        resolver.write_text('#!/bin/sh\n[ "$1" = identity ] || exit 1\n'
                            'case "$2" in rootfs-*) echo "fixture:$2" ;; *) exit 1 ;; esac\n')
        (self.root / 'tools/build_env.py').unlink()
        out = self.root / 'rootfs/out'
        out.mkdir()
        with patch.object(output, 'revision', side_effect=AssertionError('whole tree used')):
            for name in ('e2fsck', 'kexec', 'wpa_supplicant', 'wpa_cli', 'nft', 'dropbearmulti', 'sftp-server'):
                binary = out / name
                binary.write_bytes(b'executable')
                output.stamp(binary, output.source_identity(binary))
                output.verify(binary)

    def test_dependency_artifact_names_outside_rootfs_use_whole_tree_identity(self):
        binary = self.root / 'nft'
        binary.write_bytes(b'executable')
        with patch.object(output, 'revision', return_value='whole-tree'):
            output.stamp(binary, 'whole-tree')
            self.assertEqual(output.verify(binary)['tree'], 'whole-tree')

    def test_rootfs_receipts_reject_midbuild_input_changes_and_payload_replacement(self):
        binary = self.root / 'rootfs/out/nft'
        binary.parent.mkdir()
        binary.write_bytes(b'first')
        before = output.source_identity(binary)
        script = self.root / 'rootfs/build-nft.sh'
        script.write_text(script.read_text() + '\n# changed\n')
        with self.assertRaisesRegex(ValueError, 'source changed'):
            output.stamp(binary, before)
        output.stamp(binary, output.source_identity(binary))
        binary.write_bytes(b'replaced')
        with self.assertRaisesRegex(ValueError, 'stale'):
            output.verify(binary)


if __name__ == '__main__':
    unittest.main()
