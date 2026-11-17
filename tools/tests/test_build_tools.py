"""Host pins and container contracts, with fake Docker only."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import textwrap
import tomllib
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("build_env", ROOT / "tools/build_env.py")
env = importlib.util.module_from_spec(spec)
spec.loader.exec_module(env)


class Wiring(unittest.TestCase):
    def test_gpu_run_uses_daemon_platform_and_locked_image(self):
        source = (ROOT / "gpu/test-host.sh").read_text()
        self.assertIn('build-env.sh" prepare gpu-test', source)
        self.assertIn('platform gpu-test', source)
        self.assertIn('docker run --rm --pull=never --platform "$platform"', source)
        self.assertNotIn('docker image inspect', source)

    def test_builders_have_no_floating_bases_or_runtime_package_installs(self):
        names = ['boot/build-dev-image.sh', 'kernel/build-base.sh', 'gpu/build.sh', 'gpu/test-host.sh']
        names += [f'rootfs/build-{n}.sh' for n in ('wlan-tools', 'fsck', 'kexec', 'nft', 'dropbear', 'sftp-server')]
        names += [f'tools/{n}-sim.sh' for n in ('ssh', 'fw', 'usb', 'wlan', 'gel')]
        for name in names:
            with self.subTest(name=name):
                source = (ROOT / name).read_text()
                self.assertNotIn('alpine:3.22', source)
                self.assertNotIn('ubuntu:26.04', source)
                self.assertNotIn('apk add ', source)
                self.assertNotIn('apt-get install ', source)
                self.assertNotIn('docker image inspect', source)
                self.assertIn('build-env.sh', source)

    def test_kernel_and_wlan_consumers_require_environment_provenance(self):
        kernel = (ROOT / 'kernel/build-base.sh').read_text()
        image = (ROOT / 'boot/build-dev-image.sh').read_text()
        wlan = (ROOT / 'tools/wlan-sim.sh').read_text()
        self.assertIn('"$environment" > "$out/build-environment.pending"', kernel)
        self.assertIn('rm -f "$out/build-environment"', kernel)
        self.assertIn('cat "$k/build-environment"', image)
        self.assertIn('"$expected_kernel"', image)
        self.assertIn('verify "$root/rootfs/out/wpa_supplicant"', wlan)
        for name in ('fw', 'ssh'):
            self.assertIn('verify "$out/nft"', (ROOT / f'tools/{name}-sim.sh').read_text())

    def test_image_workflow_requires_existing_rustup(self):
        workflow = (ROOT / '.github/workflows/image.yml').read_text()
        script = textwrap.dedent(workflow.split('        run: |\n', 1)[1].split('      - name:', 1)[0])
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            binaries = root / '.cargo/bin'
            binaries.mkdir(parents=True)
            variables = {'HOME': str(root), 'PATH': str(binaries)}
            result = subprocess.run(['/bin/sh', '-ec', script], env=variables, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('rustup is required', result.stderr)
            for name in ('rustup', 'cargo'):
                binary = binaries / name
                binary.write_text('#!/bin/sh\nprintf "%s\\n" "' + name + ' $*"\n')
                binary.chmod(0o700)
            result = subprocess.run(['/bin/sh', '-ec', script], env=variables, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.splitlines(), ['rustup toolchain install', 'cargo fetch --locked'])

    def test_shared_pins_and_bootstrap(self):
        tools = json.loads((ROOT / 'tools/build-tools.json').read_text())
        rust = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']
        self.assertEqual(rust['channel'], '1.99.1')
        self.assertEqual(rust['targets'], ['aarch64-unknown-linux-musl'])
        self.assertFalse((ROOT / '.python-version').exists())
        setup = (ROOT / 'tools/bootstrap.sh').read_text()
        self.assertIn('cargo install cargo-llvm-cov --version "=$coverage_pin" --locked', setup)
        self.assertIn('cargo install cargo-nextest --version "=$nextest_pin" --locked', setup)
        self.assertNotIn('sudo ', setup)
        self.assertNotIn('--privileged', setup)
        self.assertEqual(tools['python'], '3.12')


class RecipePins(unittest.TestCase):
    def test_all_recipes_supply_the_contract_base_for_both_platforms(self):
        lock = env.load()
        self.assertNotIn('bases', lock)
        self.assertEqual(set(lock['roles']), {p.stem for p in (ROOT / 'tools/build-env').glob('*.Dockerfile')})
        for role in lock['roles']:
            recipe = (ROOT / f'tools/build-env/{role}.Dockerfile').read_text()
            for platform in ('linux/arm64', 'linux/amd64'):
                with self.subTest(role=role, platform=platform):
                    contract, _, _ = env.identity(role, platform)
                    name, index = recipe.splitlines()[0].removeprefix('FROM ').split('@')
                    self.assertEqual(contract['base'], name + '@' + lock['manifests'][index][platform])
                    self.assertTrue(env.digest(contract['base'].split('@')[1]))

    def test_invalid_platform_manifests_are_rejected(self):
        lock = env.load()
        index = env.base_for('initramfs').split('@')[1]
        for manifests in ({'linux/arm64': 'sha256:' + '1' * 64},
                          {'linux/arm64': 'invalid', 'linux/amd64': 'sha256:' + '2' * 64},
                          {'linux/arm64': 'sha256:' + '1' * 64, 'linux/riscv64': 'sha256:' + '2' * 64}):
            with self.subTest(manifests=manifests):
                lock['manifests'][index] = manifests
                with patch.object(env.json, 'loads', return_value=lock):
                    with self.assertRaisesRegex(env.Prerequisite, 'invalid base platform manifests'):
                        env.load()

    def test_unpinned_base_is_rejected_even_with_matching_recipe_hash(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            recipe = root / 'tools/build-env/test.Dockerfile'
            recipe.parent.mkdir(parents=True)
            for base in ('ubuntu:26.04', '@BASE@', 'alpine:3.22@sha256:invalid'):
                with self.subTest(base=base):
                    recipe.write_text('FROM ' + base + '\n')
                    lock = {'schema': 1, 'roles': {'test': {
                        'requested': ['gcc'], 'recipe_sha256': hashlib.sha256(recipe.read_bytes()).hexdigest()}}}
                    lock_path = root / 'tools/container-images.json'
                    lock_path.write_text(json.dumps(lock))
                    with patch.object(env, 'ROOT', root), patch.object(env, 'LOCK', lock_path):
                        with self.assertRaisesRegex(env.Prerequisite, 'digest-pinned base'):
                            env.load()


class Containers(unittest.TestCase):
    def setUp(self):
        self.lock = env.load()
        self.lock['roles']['gpu-test']['requested'] = ['gcc']
        self.patcher = patch.object(env, 'load', return_value=self.lock)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)
        self.contract, self.tag, self.key = env.identity('gpu-test', 'linux/arm64')
        self.info = {'Id': 'sha256:' + '2' * 64, 'Os': 'linux', 'Architecture': 'arm64',
                     'Config': {'Labels': {env.LABEL: self.key}}, 'RootFS': {'Layers': ['base', 'derived']}}
        self.base_info = {'Os': 'linux', 'Architecture': 'arm64', 'RootFS': {'Layers': ['base']}, 'RepoDigests': [self.contract['base']]}
        self.calls = []

    def docker(self, *args, **kwargs):
        self.calls.append(args)
        if args[:2] == ('image', 'inspect'):
            return json.dumps([self.info if args[-1] == self.tag else self.base_info])
        if args[:2] == ('info', '--format'):
            return 'aarch64'
        return 'gcc=15.1'

    def test_tag_only_and_architecture_only_caches_accept_wrong_image(self):
        # The parent checked only inspect success (or Architecture=arm64).
        self.assertEqual(self.info['Architecture'], 'arm64')
        self.info['Config']['Labels'][env.LABEL] = 'wrong-recipe'
        with patch.object(env, 'docker', self.docker), self.assertRaisesRegex(ValueError, 'identity mismatch'):
            env.resolve('gpu-test')

    def test_digest_platform_and_recipe_are_required(self):
        for mutation in ('platform', 'id', 'recipe'):
            with self.subTest(mutation=mutation):
                info = copy.deepcopy(self.info)
                if mutation == 'platform': info['Architecture'] = 'amd64'
                if mutation == 'id': info['Id'] = 'mutable-tag'
                if mutation == 'recipe': info['Config']['Labels'][env.LABEL] = 'old'
                self.info, previous = info, self.info
                fake = self.docker
                with patch.object(env, 'docker', fake), self.assertRaises(ValueError):
                    env.resolve('gpu-test')
                self.info = previous

    def test_correct_cache_uses_immutable_id_and_never_installs_or_pulls(self):
        with patch.object(env, 'docker', self.docker):
            self.assertEqual(env.resolve('gpu-test'), self.info['Id'])
        self.assertIn(('image', 'inspect', self.tag), self.calls)
        self.assertFalse(any(a in ('build', 'pull', '--privileged') for call in self.calls for a in call))

    def test_mutable_kernel_cache_namespace_is_local_to_the_worktree(self):
        with patch.object(env, 'ROOT', Path('/path/to/first')):
            first = env.workspace()
            self.assertEqual(first, env.workspace())
        with patch.object(env, 'ROOT', Path('/path/to/second')):
            self.assertNotEqual(first, env.workspace())
        script = (ROOT / 'kernel/build-base.sh').read_text()
        self.assertIn('oos-kbuild-$workspace-$environment', script)

    def test_kernel_platform_uses_the_daemon_architecture_and_rejects_unknown_hosts(self):
        for actual, expected in [('x86_64', 'linux/amd64'), ('aarch64', 'linux/arm64')]:
            with patch.object(env, 'docker', return_value=actual):
                self.assertEqual(env.platform_for('kernel'), expected)
        with patch.object(env, 'docker', return_value='unsupported'), self.assertRaises(ValueError):
            env.platform_for('kernel')

    def test_same_architecture_wrong_base_is_rejected(self):
        self.base_info['RepoDigests'] = ['ubuntu@sha256:' + '9' * 64]
        with patch.object(env, 'docker', self.docker), self.assertRaisesRegex(ValueError, 'base digest'):
            env.resolve('gpu-test')

    def test_offline_missing_daemon_and_unsupported_platform_fail(self):
        with patch.object(env, 'docker', side_effect=ValueError('offline')), self.assertRaises(ValueError):
            env.resolve('gpu-test')
        with patch.object(env, 'docker') as fake, self.assertRaisesRegex(ValueError, 'unsupported'):
            env.resolve('gpu-test', 'linux/riscv64')
        fake.assert_not_called()

    def test_identity_changes_for_package_requests_base_and_recipe(self):
        for change in ('recipe_sha256', 'requested'):
            original = copy.deepcopy(self.lock['roles']['gpu-test'][change])
            self.lock['roles']['gpu-test'][change] = ('f' * 64 if change == 'recipe_sha256' else ['gcc', 'g++'])
            self.assertNotEqual(env.identity('gpu-test', 'linux/arm64')[2], self.key)
            self.lock['roles']['gpu-test'][change] = original
        index = env.base_for('gpu-test').split('@')[1]
        self.lock['manifests'][index]['linux/arm64'] = 'sha256:' + '3' * 64
        self.assertNotEqual(env.identity('gpu-test', 'linux/arm64')[2], self.key)

    def test_alpine_inventory_preserves_package_names_and_revision(self):
        with patch.object(env, 'docker', return_value='build-base-0.5-r3\nlibstdc++-15.1.0_git20250421-r1'):
            self.assertEqual(env.actual_packages('rootfs-nft', 'sha256:' + '1' * 64, 'linux/arm64'),
                             ['build-base=0.5-r3', 'libstdc++=15.1.0_git20250421-r1'])

    def test_prepare_builds_requested_packages_from_digest_without_package_locks(self):
        calls = []
        with patch.object(env, 'resolve', side_effect=[ValueError('missing'), self.info['Id']]):
            with patch.object(env, 'docker', lambda *a, **k: calls.append((a, k)) or ''):
                self.assertEqual(env.prepare('gpu-test', 'linux/arm64'), self.info['Id'])
        self.assertEqual(calls[0][0], ('pull', '--platform', 'linux/arm64', self.contract['base']))
        args, kwargs = calls[1]
        self.assertIn('--platform', args)
        self.assertEqual(args[args.index('--platform') + 1], 'linux/arm64')
        recipe = kwargs['input']
        self.assertIn(self.contract['base'], recipe)
        self.assertIn('install -y --no-install-recommends gcc', recipe)
        self.assertNotIn('@PACKAGES@', recipe)
        self.assertIn(env.LABEL, recipe)

    def test_base_and_recipe_pins_are_usable_without_deferred_locks(self):
        self.assertNotIn('binfmt', self.lock)
        for role in self.lock['roles']:
            self.assertNotIn('packages', self.lock['roles'][role])
            contract, _, _ = env.identity(role, 'linux/arm64')
            self.assertIn('@sha256:', contract['base'])
        contract, _, _ = env.identity('kernel', 'linux/amd64')
        self.assertIn('gcc-aarch64-linux-gnu', contract['requested'])

    def test_package_inventory_is_informational_sorted_and_adjacent(self):
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / 'bundle.packages.txt'
            for packages in ('zlib=1.3\ngcc=15.1', 'gcc=16.0\nzlib=1.4'):
                with patch.object(env, 'docker', return_value=packages):
                    env.provenance('gpu-test', self.info['Id'], 'linux/arm64', output)
                self.assertEqual(output.read_text(), '\n'.join(sorted(packages.splitlines())) + '\n')
            self.assertEqual(list(output.parent.iterdir()), [output])

    def test_builder_outputs_include_package_provenance(self):
        names = ['boot/build-dev-image.sh', 'gpu/build.sh', 'gpu/test-host.sh', 'kernel/build-base.sh']
        names += [f'rootfs/build-{n}.sh' for n in ('wlan-tools', 'fsck', 'kexec', 'nft', 'dropbear', 'sftp-server')]
        for name in names:
            source = (ROOT / name).read_text()
            self.assertIn('build-env.sh" provenance ', source, name)
            self.assertIn('--image "$image" --output "$out/', source, name)


class Bootstrap(unittest.TestCase):
    def test_locked_install_and_selected_roles_without_real_installation(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / 'tools').mkdir()
            (root / 'bin').mkdir()
            for name in ('tools/bootstrap.sh', 'tools/build-tools.json', 'tools/output.sh', 'tools/output.py', 'rust-toolchain.toml'):
                dest = root / name
                dest.write_bytes((ROOT / name).read_bytes())
            log = root / 'commands'
            for name in ('cargo', 'rustup'):
                path = root / 'bin' / name
                path.write_text('#!/bin/sh\nprintf "%s\\n" "' + name + ' $*" >> "$BOOTSTRAP_LOG"\n')
                path.chmod(0o700)
            resolver = root / 'tools/build-env.sh'
            resolver.write_text('#!/bin/sh\nprintf "%s\\n" "environment $*" >> "$BOOTSTRAP_LOG"\n')
            resolver.chmod(0o700)
            variables = {**os.environ, 'BOOTSTRAP_LOG': str(log), 'PATH': str(root / 'bin') + os.pathsep + os.environ['PATH']}
            variables.pop('CARGO_TARGET_DIR', None)
            result = subprocess.run(['sh', str(root / 'tools/bootstrap.sh'), '--image'], env=variables, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            lines = log.read_text().splitlines()
            self.assertIn('cargo install cargo-llvm-cov --version =0.9.1 --locked', lines)
            self.assertIn('cargo install cargo-nextest --version =0.9.150 --locked', lines)
            self.assertIn('rustup toolchain install 1.99.1 --profile minimal --component rustfmt,clippy,llvm-tools-preview --target aarch64-unknown-linux-musl', lines)
            self.assertIn('cargo fetch --locked', lines)
            self.assertEqual(lines[-1], 'cargo xtask doctor --image')
            roles = json.loads((ROOT / 'tools/build-tools.json').read_text())['profiles']
            expected = set(r for names in roles.values() for r in names)
            actual = {line.removeprefix('environment prepare ') for line in lines if line.startswith('environment prepare ')}
            self.assertEqual(actual, expected)
            log.unlink()
            invalid = subprocess.run(['sh', str(root / 'tools/bootstrap.sh'), '--install'], env=variables, capture_output=True)
            self.assertEqual(invalid.returncode, 2)
            self.assertFalse(log.exists())


if __name__ == '__main__':
    unittest.main()
