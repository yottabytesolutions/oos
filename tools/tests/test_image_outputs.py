# SPDX-License-Identifier: GPL-2.0-or-later
"""Image packaging contract, using private files without Docker, sockets or Git commits."""
import os
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

REPO = Path(__file__).resolve().parents[2]
GEL = "DHO800_DHO900_OOS_Install.GEL"


class ImageOutputs(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name).resolve()
        for name in ("boot/build-gel.py", "boot/boardconf.py", "boot/gel/do_extract.sh",
                     "rootfs/board/dho/etc/oos-board", "tools/output.py", "tools/output.sh"):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / name, dest)
        # Only revision discovery is a stand-in. Actual path resolution, receipts and archives run unchanged.
        git = self.root / "bin/git"
        git.parent.mkdir()
        git.write_text('#!/bin/sh\ncase "$*" in *rev-parse*) echo fixture;; *ls-files*) ;; *) exit 1;; esac\n')
        git.chmod(0o700)
        # Packaging receipts need compiler identities, but these fixtures never compile Rust.
        for name in ("cargo", "rustc"):
            compiler = git.parent / name
            compiler.write_text(f'#!/bin/sh\nprintf "{name} 1.99.1 fixture\\n"\n')
            compiler.chmod(0o700)
        self.env = {**os.environ, "PATH": str(git.parent) + os.pathsep + os.environ["PATH"]}
        self.env.pop("CARGO_TARGET_DIR", None)
        self.env.pop("OOS_BOARD", None)

    def run_python(self, script, *args, ok=True):
        result = subprocess.run([sys.executable, "-I", str(self.root / script), *map(str, args)],
                                cwd=self.root.parent, env=self.env, capture_output=True, text=True)
        if ok:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
        return result

    def prepare(self, selected="isolated target"):
        if selected is not None:
            self.env["CARGO_TARGET_DIR"] = selected
        target = (self.root / (selected or "target")).resolve()
        package = target / "image/oos-dev.oosupd"
        updater = target / "aarch64-unknown-linux-musl/release/oos-update"
        for path, payload in ((package, b"selected signed package"), (updater, b"selected updater")):
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(payload)
            revision = self.run_python("tools/output.py", "revision").stdout.strip()
            self.run_python("tools/output.py", "stamp", path, revision)
        return target, package, updater

    def test_default_relative_and_absolute_targets(self):
        for selected in (None, "isolated target", str(self.root / "absolute target")):
            with self.subTest(selected=selected):
                target, package, updater = self.prepare(selected)
                self.run_python("boot/build-gel.py")
                gel = target / "image" / GEL
                self.run_python("tools/output.py", "verify", gel)
                with tarfile.open(gel, "r:gz") as archive:
                    self.assertEqual(archive.extractfile("oos/oos.oosupd").read(), package.read_bytes())
                    self.assertEqual(archive.extractfile("oos/oos-update").read(), updater.read_bytes())
                    self.assertEqual(archive.getmember("oos/oos-update").mode, 0o755)
                    script = archive.extractfile("shell/do_extract.sh").read()
                    self.assertNotIn(b"@OOS_GEL_STEM@", script)
                    self.assertIn(b"DHO800_DHO900_OOS", script)
                self.assertFalse((self.root / "boot/out").exists())

    def test_legacy_outputs_cannot_fill_missing_selected_inputs(self):
        target, package, _ = self.prepare()
        legacy = self.root / "boot/out/oos-dev.oosupd"
        legacy.parent.mkdir(parents=True)
        shutil.copyfile(package, legacy)
        legacy_tool = self.root / "target/aarch64-unknown-linux-musl/release/oos-update"
        legacy_tool.parent.mkdir(parents=True)
        legacy_tool.write_bytes(b"legacy updater")
        package.unlink()
        self.assertIn("missing", self.run_python("boot/build-gel.py", ok=False).stderr)
        self.assertFalse((target / "image" / GEL).exists())

    def test_ui_builder_uses_receipted_selected_outputs(self):
        for name in ("ui/build.sh", "ui/Cargo.toml", "tools/output.sh", "ui/tools/decoder-features.sh"):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(REPO / name, dest)
        assets = self.root / "ui/assets"
        assets.mkdir()
        for name in ("font.bin", "font-large.bin", "font-larger.bin"):
            (assets / name).write_bytes(b"font fixture")
        cargo = self.root / "bin/cargo"
        cargo.write_text(
            '#!' + sys.executable + '\n'
            'import json, os, pathlib, sys\n'
            'if "--version" in sys.argv: print("cargo 1.99.1 fixture"); sys.exit(0)\n'
            'p = pathlib.Path(os.environ["CARGO_TARGET_DIR"])\n'
            'if "--target" in sys.argv: p /= sys.argv[sys.argv.index("--target") + 1]\n'
            'p /= "release/oos-ui"\n'
            'p.parent.mkdir(parents=True, exist_ok=True)\n'
            'p.write_bytes(b"fixture executable")\n'
            'print(json.dumps({"reason": "compiler-artifact", "executable": str(p)}))\n')
        cargo.chmod(0o700)
        rustc = self.root / "bin/rustc"
        rustc.write_text('#!/bin/sh\nprintf "rustc 1.99.1 fixture\\n"\n')
        rustc.chmod(0o700)
        # The build script prepends the user's Cargo directory; keep this test entirely in its fixture.
        self.env["HOME"] = str(self.root / "home")
        self.env["CARGO_TARGET_DIR"] = "selected target"
        result = subprocess.run(["sh", str(self.root / "ui/build.sh")], cwd=self.root.parent,
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in ("release/oos-ui", "aarch64-unknown-linux-musl/release/oos-ui"):
            self.run_python("tools/output.py", "verify", self.root / "selected target" / name)
        self.assertFalse((self.root / "ui/selected target").exists())

    def test_missing_receipts_and_replaced_inputs_fail_closed(self):
        for which in (1, 2):
            for replaced in (False, True):
                with self.subTest(which=which, replaced=replaced):
                    paths = self.prepare()
                    target, path = paths[0], paths[which]
                    if replaced:
                        path.write_bytes(b"replaced payload")
                    else:
                        path.with_name(path.name + ".oos-build.json").unlink()
                    self.run_python("boot/build-gel.py", ok=False)
                    self.assertFalse((target / "image" / GEL).exists())

    def run_guard(self, *args, fail=False):
        shutil.copyfile(REPO / "tools/perf-guard.sh", self.root / "tools/perf-guard.sh")
        (self.root / "tools/perf-budgets.py").write_text(
            "import os\nfrom pathlib import Path\nPath(os.environ['OOS_GUARD_BUDGET']).touch()\n")
        (self.root / "record.py").write_text(
            "import json, os, sys\n"
            "with open(os.environ['OOS_GUARD_CALLS'], 'a') as out:\n"
            " out.write(json.dumps([sys.argv[1:], os.environ['CARGO_TARGET_DIR'], "
            "os.environ['OOS_PERF_TESTS']]) + '\\n')\n"
            "sys.exit(int(os.environ['OOS_GUARD_EXIT']))\n")
        calls = self.root / "guard-calls"
        budget = self.root / "guard-budget"
        calls.unlink(missing_ok=True)
        budget.unlink(missing_ok=True)
        self.env.update(CARGO_TARGET_DIR="selected target", OOS_GUARD_CALLS=str(calls),
                        OOS_GUARD_BUDGET=str(budget), OOS_GUARD_EXIT="7" if fail else "0")
        # A function substitutes Cargo even after the script prepends the real Cargo bin to PATH.
        command = 'cargo() { python3 "$root/record.py" "$@"; }; . "$0"'
        result = subprocess.run(["sh", "-c", command, str(self.root / "tools/perf-guard.sh"), *args],
                                cwd=self.root.parent, env=self.env, capture_output=True, text=True)
        records = [json.loads(line) for line in calls.read_text().splitlines()]
        self.assertTrue(records)
        for _, target, enabled in records:
            self.assertEqual(Path(target), self.root / "selected target/perf-build")
            self.assertEqual(enabled, "1")
        return result, [r[0] for r in records], budget.exists()

    def test_perf_guard_exports_root_relative_target_before_changing_directory(self):
        result, _, _ = self.run_guard()
        self.assertEqual(result.returncode, 0, result.stderr)

    @staticmethod
    def targets(calls):
        targets = set()
        for args in calls:
            profile = args[args.index("--profile") + 1] if "--profile" in args else ("release" if "--release" in args else "dev")
            package = args[args.index("-p") + 1]
            for index, arg in enumerate(args):
                if arg in ("--test", "--bin"):
                    targets.add((profile, package, arg, args[index + 1]))
                elif arg == "--lib":
                    targets.add((profile, package, arg, ""))
        return targets

    def test_preparation_only_compiles_every_target_used_by_the_guards(self):
        built, builds, checked = self.run_guard("--build-only")
        self.assertEqual(built.returncode, 0, built.stderr)
        self.assertFalse(checked)
        self.assertEqual(len(builds), 2)
        self.assertTrue(all("--no-run" in args for args in builds))
        ran, runs, checked = self.run_guard()
        self.assertEqual(ran.returncode, 0, ran.stderr)
        self.assertTrue(checked)
        self.assertEqual(len(runs), 5)
        self.assertFalse(any("--no-run" in args for args in runs))
        self.assertEqual(self.targets(builds), self.targets(runs))
        self.assertEqual(self.targets(runs), {
            ("perf", "oos-hal", "--lib", ""),
            ("perf", "oos-hal", "--test", "alloc_batch"),
            ("perf", "oos-hal", "--test", "early_trigger_end"),
            ("perf", "oos-hal", "--test", "perf_ratio"),
            ("release", "oos-ui", "--test", "soak"),
            ("release", "oos-ui", "--bin", "oos-ui"),
        })
        timing = [args for args in runs if "--ignored" in args]
        self.assertEqual(len(timing), 2)
        self.assertTrue(all("--test-threads=1" in args for args in timing))

    def test_a_failed_build_fails_preparation_without_running_the_guards(self):
        result, calls, checked = self.run_guard("--build-only", fail=True)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(len(calls), 1)
        self.assertFalse(checked)


    @unittest.skipIf(os.environ.get("OOS_PYTHON_NO_RUST") == "1" or not shutil.which("cargo"),
                     "requires cargo clean to check GPU cache survival; unavailable in Python-only CI")
    def test_gpu_cache_survives_cargo_clean(self):
        script = self.root / "gpu/build.sh"
        script.parent.mkdir()
        shutil.copyfile(REPO / "gpu/build.sh", script)
        for name in ("build-env.sh", "build_env.py", "container-images.json", "lib/portable.sh"):
            (self.root / "tools" / name).parent.mkdir(exist_ok=True)
            shutil.copyfile(REPO / "tools" / name, self.root / "tools" / name)
        (self.root / "tools/build-env.sh").chmod(0o700)
        shutil.copytree(REPO / "tools/build-env", self.root / "tools/build-env")
        lock_path = self.root / "tools/container-images.json"
        lock = json.loads(lock_path.read_text())
        key = self.run_python("tools/build_env.py", "identity", "gpu-build").stdout.strip()
        self.env['OOS_TEST_ENVIRONMENT'] = key
        docker = self.root / "bin/docker"
        docker.write_text(
            '#!' + sys.executable + '\n'
            'import pathlib, sys, os, json\n'
            'key = os.environ["OOS_TEST_ENVIRONMENT"]\n'
            'if sys.argv[1:3] == ["image", "inspect"]:\n'
            '    if "@sha256:" in sys.argv[-1]:\n'
            '        print(json.dumps([{"Os": "linux", "Architecture": "arm64", "RootFS": {"Layers": ["base"]}, "RepoDigests": [sys.argv[-1]]}])); sys.exit(0)\n'
            '    print(json.dumps([{"Id": "sha256:" + "2" * 64, "Os": "linux", "Architecture": "arm64", "RootFS": {"Layers": ["base", "derived"]}, "Config": {"Labels": {"org.oos.environment": key}}}])); sys.exit(0)\n'
            'if "--network=none" in sys.argv:\n'
            '    print("zlib=1.3\\ngcc=15.1"); sys.exit(0)\n'
            'mounts = dict(a.rsplit(":", 1)[::-1] for a in sys.argv if a.endswith((":/out", ":/cache")))\n'
            'cache = pathlib.Path(mounts["/cache"])\n'
            'if "/out" not in mounts:\n'
            '    (cache / "mesa_26.0.8.orig.tar.xz").write_bytes(b"tarball fixture")\n'
            'else:\n'
            '    (cache / "mesa" / key).mkdir(parents=True, exist_ok=True)\n'
            '    marker = cache / "mesa" / key / "built"\n'
            '    if not marker.exists(): marker.write_bytes(b"compiled Mesa")\n'
            '    bundle = pathlib.Path(mounts["/out"]) / "gpu"\n'
            '    bundle.mkdir(exist_ok=True)\n'
            '    (bundle / "oos-gpu").write_bytes(marker.read_bytes())\n')
        docker.chmod(0o700)
        # the fixture tarball passes the pinned checksum: the hash tool answers with the pinned sum
        pinned = next(line[4:] for line in script.read_text().splitlines() if line.startswith("sum="))
        checksum = self.root / "bin/sha256sum"
        checksum.write_text(f'#!/bin/sh\ncat "$@" >/dev/null\necho "{pinned}  -"\n')
        checksum.chmod(0o700)
        target = self.root / "selected target"
        self.env["CARGO_TARGET_DIR"] = str(target)
        result = subprocess.run(["sh", str(script)], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((target / "gpu/build-packages.txt").read_text(), "gcc=15.1\nzlib=1.3\n")
        cache = self.root / "rootfs/out/mesa" / key / "built"
        before = cache.stat().st_mtime_ns
        self.run_python("tools/output.py", "verify", target / "gpu/gpu")
        # Clean a private fixture target, never the contributor's own build outputs.
        manifest = self.root / "Cargo.toml"
        manifest.write_text('[package]\nname="cache-check"\nversion="0.1.0"\n[[bin]]\nname="cache-check"\npath="main.rs"\n')
        (self.root / "main.rs").write_text("fn main() {}\n")
        clean = subprocess.run([shutil.which("cargo"), "clean", "--manifest-path", str(manifest)],
                               env={**self.env, "PATH": os.environ["PATH"]}, capture_output=True, text=True)
        self.assertEqual(clean.returncode, 0, clean.stderr)
        self.assertFalse((target / "gpu/gpu").exists())
        self.assertEqual(cache.stat().st_mtime_ns, before)
        result = subprocess.run(["sh", str(script)], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(cache.stat().st_mtime_ns, before)
        self.run_python("tools/output.py", "verify", target / "gpu/gpu")
        # A changed package request selects a new cache without deleting the old one.
        old_key = key
        lock['roles']['gpu-build']['requested'].append('patch')
        lock_path.write_text(json.dumps(lock))
        key = self.run_python("tools/build_env.py", "identity", "gpu-build").stdout.strip()
        self.assertNotEqual(key, old_key)
        self.env['OOS_TEST_ENVIRONMENT'] = key
        result = subprocess.run(["sh", str(script)], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(cache.stat().st_mtime_ns, before)
        self.assertTrue((self.root / "rootfs/out/mesa" / key / "built").exists())


class ElfValidation(unittest.TestCase):
    def test_shipped_binaries_use_xtask_and_propagate_validation_failure(self):
        sys.path.insert(0, str(REPO / 'tools'))
        import image_validate
        with tempfile.TemporaryDirectory() as scratch:
            target = Path(scratch)
            sources = [target / 'aarch64-unknown-linux-musl/release' / name
                       for name in ('oos-ui', 'oos-fpgad', 'oos-update')]
            sources.append(target / 'hw/oos-hw')
            (target / 'gpu/gpu').mkdir(parents=True)
            entries = {name: (0o100755, b'fixture') for name in
                       ('init', 'etc/rcS', 'bin/busybox', 'bin/oos-gpu', 'lib/ld-linux-aarch64.so.1')}
            for source in sources:
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_bytes(b'fixture')
                source.chmod(0o755)
                entries[f'bin/{source.name}'] = (0o100755, b'fixture')
            with patch.object(image_validate.output, 'verify'), patch.object(image_validate.subprocess, 'run') as run:
                image_validate.shipped(entries, target)
                self.assertEqual([call.args[0] for call in run.call_args_list],
                                 [['cargo', 'xtask', 'elf-static', str(source)] for source in sources])
                self.assertTrue(all(call.kwargs == {'cwd': REPO, 'check': True} for call in run.call_args_list))
                run.side_effect = subprocess.CalledProcessError(1, ['cargo', 'xtask', 'elf-static'])
                with self.assertRaises(subprocess.CalledProcessError):
                    image_validate.shipped(entries, target)


if __name__ == "__main__":
    unittest.main()
