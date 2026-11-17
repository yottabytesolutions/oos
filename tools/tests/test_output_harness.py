# SPDX-License-Identifier: GPL-2.0-or-later
"""Offline isolation regressions; no sockets, commits or Docker required."""
import json
import importlib.util
import io
import os
from contextlib import redirect_stderr
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import output
from host_harness import Host, HostError, wait_ready


class Outputs(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()

    def test_targets_resolve_from_worktree_and_reject_escape(self):
        for value in ("target/custom", str(self.root / "target/custom")):
            with patch.dict(os.environ, CARGO_TARGET_DIR=value):
                self.assertEqual(output.target(self.root), self.root / "target/custom")
        for value in ("..", str(self.root), str(self.root.parent / "elsewhere")):
            with patch.dict(os.environ, CARGO_TARGET_DIR=value), self.assertRaises(ValueError):
                output.target(self.root)
        (self.root / "escape").symlink_to(self.root.parent, target_is_directory=True)
        with patch.dict(os.environ, CARGO_TARGET_DIR="escape/out"), self.assertRaises(ValueError):
            output.target(self.root)

    def test_compiler_changes_invalidate_shipped_binary_receipts(self):
        binary = self.root / "oos-ui"
        binary.write_bytes(b"same executable")
        with patch.object(output, "revision", return_value="tree"):
            with patch.object(output, "compiler_identity", create=True, return_value="first compiler"):
                output.stamp(binary, "tree")
                output.verify(binary)
            with patch.object(output, "compiler_identity", create=True, return_value="second compiler"):
                with self.assertRaisesRegex(ValueError, "stale"):
                    output.verify(binary)

    def test_build_fetches_locked_dependencies_unless_offline_requested(self):
        binary = self.root / "oos-fpgad"
        cache = self.root / "dependencies"

        def launch(args, **kwargs):
            self.assertIn("--locked", args)
            if not cache.exists() and "--offline" in args:
                return Mock(stdout=[], wait=Mock(return_value=1))
            cache.mkdir(exist_ok=True)
            binary.write_bytes(b"built executable")
            event = json.dumps({"reason": "compiler-artifact", "executable": str(binary)})
            return Mock(stdout=[event], wait=Mock(return_value=0))

        with patch.object(sys, "argv", ["output.py", "build", "-p", "oos-fpgad", "--offline"]), \
                patch.dict(os.environ, CARGO_TARGET_DIR="target"), \
                patch.object(output, "ROOT", self.root), \
                patch.object(output, "revision", return_value="tree"), \
                patch.object(output, "compiler_identity", return_value="compiler"), \
                patch.object(output.subprocess, "Popen", side_effect=launch):
            with self.assertRaisesRegex(ValueError, "Cargo build failed; no receipts written"):
                output.main()
            self.assertFalse(cache.exists())
            self.assertFalse(output.receipt(binary).exists())
            sys.argv.pop()
            output.main()
            self.assertTrue(cache.is_dir())
            output.verify(binary)

    def test_receipts_reject_replaced_payload_source_and_missing_stamp(self):
        binary = self.root / "program"
        binary.write_bytes(b"first build")
        with patch.object(output, "revision", return_value="tree"):
            with self.assertRaisesRegex(ValueError, "missing build receipt.*rebuild"):
                output.verify(binary)
            output.stamp(binary, "tree")
            output.verify(binary)
            binary.write_bytes(b"replacement")
            with self.assertRaises(ValueError):
                output.verify(binary)
            output.stamp(binary, "tree")
        with patch.object(output, "revision", return_value="changed"):
            with self.assertRaises(ValueError):
                output.verify(binary)
            with self.assertRaises(ValueError):
                output.stamp(binary, "tree")

    def test_bundle_hash_covers_libraries_and_permissions(self):
        bundle = self.root / "bundle"
        bundle.mkdir()
        library = bundle / "lib.so"
        library.write_bytes(b"original")
        before = output.digest(bundle)
        library.chmod(0o755)
        self.assertNotEqual(before, output.digest(bundle))
        before = output.digest(bundle)
        library.write_bytes(b"replaced")
        self.assertNotEqual(before, output.digest(bundle))

    def test_revision_covers_build_inputs_without_head_or_docs(self):
        source = self.root / "source.rs"
        source.write_text("original")
        excluded = ("COMMIT_MSG.txt", "COMMIT_MSG-review.txt", "docs/tasks/0026.md",
                    "nested/README.md", ".github/workflows/build.yml")
        for name in excluded:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("original")
        out = self.root / "custom"
        out.mkdir()
        (out / "generated").write_text("output")
        head = b"head"
        paths = [b"source.rs", b"custom/generated", *map(os.fsencode, excluded)]
        def git(args):
            return head if "rev-parse" in args else b"\0".join(paths)
        with patch.dict(os.environ, CARGO_TARGET_DIR="custom"), patch.object(output, "ROOT", self.root), \
                patch.object(output.subprocess, "check_output", side_effect=git):
            before = output.revision(self.root)
            binary = out / "program"
            binary.write_bytes(b"built executable")
            output.stamp(binary, before)
            (out / "generated").write_text("changed output")
            for name in excluded:
                (self.root / name).write_text("changed excluded input")
            self.assertEqual(before, output.revision(self.root))
            head = b"new head"
            self.assertEqual(before, output.revision(self.root))
            output.verify(binary)
            head = b"head"
            font = self.root / "ui/assets/font.bin"
            font.parent.mkdir(parents=True, exist_ok=True)
            font.write_bytes(b"glyphs")
            paths.append(b"ui/assets/font.bin")
            with_font = output.revision(self.root)
            self.assertNotEqual(before, with_font)
            font.write_bytes(b"changed glyphs")
            self.assertNotEqual(with_font, output.revision(self.root))
            font.unlink()
            paths.pop()
            source.write_text("edited source")
            self.assertNotEqual(before, output.revision(self.root))
            with self.assertRaisesRegex(ValueError, "stale"):
                output.verify(binary)
            source.unlink()
            self.assertNotEqual(before, output.revision(self.root))
            deleted = output.revision(self.root)
            output.stamp(binary, deleted)
            paths.remove(b"source.rs")  # Committing the deletion drops it from ls-files.
            output.verify(binary)


class Readiness(unittest.TestCase):
    def test_reports_survive_host_cleanup(self):
        with tempfile.TemporaryDirectory() as root:
            with patch("host_harness.output.target", return_value=Path(root)), \
                    patch("host_harness.output.verify", return_value={"sha256": "abc"}), \
                    patch("host_harness.subprocess.Popen"), patch("host_harness.wait_ready", return_value={}):
                with Host() as host:
                    report = Path(host.results_dir())
                    run = Path(host.temp.name)
                    (report / "result").write_text("retained")
                self.assertFalse(run.exists())
                self.assertEqual((report / "result").read_text(), "retained")

    def test_startup_failure_preserves_stderr_before_cleanup(self):
        with tempfile.TemporaryDirectory() as root:
            proc = Mock(pid=123)
            proc.poll.return_value = 1
            def launch(*args, **kwargs):
                kwargs["stderr"].write("VXI-11 bind failed: permission denied\n")
                kwargs["stderr"].flush()
                return proc
            with patch("host_harness.output.target", return_value=Path(root)), \
                    patch("host_harness.output.verify", return_value={"sha256": "abc"}), \
                    patch("host_harness.subprocess.Popen", side_effect=launch):
                with self.assertRaisesRegex(HostError, "permission denied"):
                    with Host():
                        self.fail("failed startup must not start tests")
            self.assertEqual(list(Path(root).iterdir()), [])

    def test_only_the_expected_child_build_and_ports_are_accepted(self):
        proc = Mock(pid=123)
        proc.poll.return_value = None
        with tempfile.TemporaryDirectory() as run:
            path = Path(run) / "ready.json"
            ready = dict(pid=123, build="abc", web=1001, scpi=1002, vxi_tcp=1003, vxi_udp=1004, vxi_core=1005)
            path.write_text(json.dumps(ready))
            self.assertEqual(wait_ready(proc, path, "abc"), ready)
            for key, value in (("pid", 456), ("build", "old"), ("scpi", 0), ("vxi_core", None)):
                path.write_text(json.dumps({**ready, key: value}))
                with self.assertRaises(ValueError):
                    wait_ready(proc, path, "abc")
            path.unlink()
            with self.assertRaisesRegex(ValueError, "deadline"):
                wait_ready(proc, path, "abc", timeout=0)
            proc.poll.return_value = 1
            with self.assertRaisesRegex(ValueError, "exited"):
                wait_ready(proc, path, "abc")


class DeviceRunner(unittest.TestCase):
    def test_only_expected_harness_failures_are_reported_as_startup_errors(self):
        spec = importlib.util.spec_from_file_location("device_test", Path(__file__).resolve().parents[1] / "device-test.py")
        runner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runner)
        for error in (ValueError("test bug"), OSError("test IO bug"), HostError("startup cause")):
            with patch.object(sys, "argv", ["device-test.py", "--sim"]), \
                    patch.object(runner.host_harness, "Host"), \
                    patch.object(runner, "run_tests", side_effect=error), redirect_stderr(io.StringIO()) as stderr:
                if isinstance(error, HostError):
                    with self.assertRaises(SystemExit) as exit:
                        runner.main()
                    self.assertEqual(exit.exception.code, 1)
                    self.assertIn("startup cause", stderr.getvalue())
                else:
                    with self.assertRaises(type(error)):
                        runner.main()


if __name__ == "__main__":
    unittest.main()
