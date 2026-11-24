# SPDX-License-Identifier: GPL-2.0-or-later
"""Offline packaging and trust checks with temporary Ed25519 keys."""
import contextlib
import gzip
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("build_release", ROOT / "tools/build-release.py")
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)

BUILD = "20260101-120000 kernel 6.12.115+"
MODELS = "DHO804 DHO924"
COMMIT = "0123456789abcdef" * 2 + "01234567"
UPDATER_BYTES = b"fixture stock installer"
IMAGE_NAME = "oos-v0.2.0.img"
PACKAGE_NAME = "oos-v0.2.0.oosupd"
GEL_NAME = "DHO800_DHO900_OOS_Install.GEL"
BRIDGE_NAME = f"oos-v0.2.0-{release.BRIDGE_LABEL}.oosupd"
HOST_UPDATER = os.environ.get("OOS_RELEASE_TEST_UPDATER")
CHECKOUT_GIT = ["git", "--no-optional-locks", "--git-dir=.git", "--work-tree=."]


class ReleaseKeyInputs(unittest.TestCase):
    def test_missing_key_fails_before_using_image_or_updater(self):
        with patch.dict(os.environ, {"OOS_SIGNING_KEY": ""}):
            result = subprocess.run([sys.executable, "-I", SPEC.origin, "--image", "missing.img",
                                     "--tag", "v0.2.0", "--commit", COMMIT, "--updater", "missing",
                                     "--out", "unused"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("signing requires --key FILE or OOS_SIGNING_KEY", result.stderr)

    def test_repository_key_path_is_refused(self):
        with self.assertRaisesRegex(ValueError, "outside the repository"):
            release._external_key(ROOT / "keys/update.pub")


class ReleaseCheckout(unittest.TestCase):
    def test_commit_must_match_checkout_head(self):
        with patch.object(release.subprocess, "run", return_value=subprocess.CompletedProcess(
                [], 0, stdout=COMMIT.encode() + b"\n")) as git:
            with self.assertRaisesRegex(ValueError, "--commit must match checkout HEAD"):
                release._require_clean_checkout("0" * 40)
        git.assert_called_once()
        self.assertEqual(git.call_args.args[0], [*CHECKOUT_GIT, "rev-parse", "--verify", "HEAD"])

    def test_checkout_must_be_clean(self):
        for status in (b"M  staged\0", b" M edited\0", b"?? tools/new-source\0"):
            with self.subTest(status=status), patch.object(release.subprocess, "run", side_effect=[
                subprocess.CompletedProcess([], 0, stdout=COMMIT.encode() + b"\n"),
                subprocess.CompletedProcess([], 0, stdout=status),
            ]) as git, patch.dict(os.environ, {"GIT_DIR": "unrelated", "GIT_INDEX_FILE": "unrelated"}):
                with self.assertRaisesRegex(ValueError, "release checkout must be clean"):
                    release._require_clean_checkout(COMMIT)
                self.assertEqual(git.call_args.args[0], [*CHECKOUT_GIT, "status", "--porcelain=v1",
                                                        "-z", "--untracked-files=all", "--ignore-submodules=none"])
                self.assertEqual(git.call_args.kwargs["cwd"], ROOT)
                self.assertNotIn("GIT_DIR", git.call_args.kwargs["env"])
                self.assertNotIn("GIT_INDEX_FILE", git.call_args.kwargs["env"])


@unittest.skipUnless(HOST_UPDATER, "set OOS_RELEASE_TEST_UPDATER to the built host oos-update")
class ReleaseArtifacts(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.image = self.root / "input.img"
        self.updater = Path(HOST_UPDATER).resolve()
        self.release_key = self.root / "release.key"
        self.community_key = self.root / "upstream/keys/community-signing.key"
        self.release_public, self.release_id = self.keygen(self.release_key)
        self.community_public, self.community_id = self.keygen(self.community_key)
        self.write_image()
        self.signer = self.root / "fixture-updater"
        self.signer.write_text("""#!/bin/sh
command=$1
shift
case "$command" in
    pack)
        for arg do
            [ "$arg" != --untrusted-ok ] || exec "$TEST_UPDATER" pack "$@"
        done
        exec "$TEST_UPDATER" pack "$@" --untrusted-ok
        ;;
    verify)
        if [ "$#" -eq 1 ]; then
            set -- "$@" --keys "$TEST_RELEASE_PUBLIC"
        fi
        exec "$TEST_UPDATER" verify "$@"
        ;;
    *) exit 2 ;;
esac
""")
        self.signer.chmod(0o700)

    def keygen(self, key):
        result = subprocess.run([self.updater, "keygen", "--out", key], check=True,
                                capture_output=True, text=True)
        public, _, _, key_id = result.stdout.splitlines()[-1].split()
        path = key.with_suffix(".pub")
        path.write_text(public + "\n", encoding="ascii")
        return path, key_id

    def write_image(self):
        boot = release.boot_module()
        kernel = bytearray(4096)
        kernel[0x38:0x3c] = b"ARM\x64"
        # Version 17 header, empty reservation table and a root node with no properties.
        dtb = struct.pack(">10I", 0xD00DFEED, 72, 56, 72, 40, 17, 16, 0, 0, 16)
        dtb += bytes(16) + struct.pack(">4I", 1, 0, 2, 9)
        archive = boot.cpio([("etc/oos-build", 0o100644, BUILD.encode() + b"\n"),
                             ("etc/oos-models", 0o100644, MODELS.encode() + b"\n"),
                             ("bin/oos-update", 0o100755, UPDATER_BYTES)])
        with contextlib.redirect_stderr(io.StringIO()):
            self.image.write_bytes(boot.build(bytes(kernel), dtb, gzip.compress(archive, mtime=0)))

    def make_release(self, out, *, signer=None, bridge=False, environment=False):
        argv = ["--image", str(self.image), "--tag", "v0.2.0", "--commit", COMMIT,
                "--updater", str(signer or self.signer), "--out", str(out)]
        if not environment:
            argv.extend(["--key", str(self.release_key)])
        if bridge:
            argv.extend(["--community-key", str(self.community_key)])
        env = {"TEST_UPDATER": str(self.updater), "TEST_RELEASE_PUBLIC": str(self.release_public),
               "OOS_SIGNING_KEY": str(self.release_key if environment else self.root / "unused")}
        # The package fixtures are independent of the developer's working tree.
        with patch.dict(os.environ, env), patch.object(release, "_checkout_git",
                                                       side_effect=[COMMIT.encode() + b"\n", b""]):
            release.main(argv)

    def check_package(self, package, key, public):
        header, image = package.read_bytes().split(b"\n\n", 1)
        self.assertEqual(image, self.image.read_bytes())
        self.assertIn(b"models " + MODELS.encode(), header.splitlines())
        self.assertIn(b"trial kexec", header.splitlines())
        self.assertIn(b"key " + key.encode(), header.splitlines())
        result = subprocess.run([self.updater, "verify", package, "--keys", public],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_release_and_bridge_are_byte_reproducible(self):
        first, second = self.root / "first", self.root / "second"
        with patch.object(release, "COMMUNITY_PUBLIC_KEY", self.community_public.read_text().strip()):
            self.make_release(first, bridge=True)
            renamed = self.root / "different input.img"
            shutil.copyfile(self.image, renamed)
            os.utime(renamed, (1, 1))
            self.image = renamed
            self.make_release(second, bridge=True, environment=True)
        expected = {IMAGE_NAME, PACKAGE_NAME, GEL_NAME, BRIDGE_NAME, "release-manifest.json", "SHA256SUMS"}
        self.assertEqual({p.name for p in first.iterdir()}, expected)
        self.assertEqual({p.name for p in second.iterdir()}, expected)
        for name in expected:
            self.assertEqual((first / name).read_bytes(), (second / name).read_bytes(), name)
        sums = (first / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(sums, [f"{hashlib.sha256((first / name).read_bytes()).hexdigest()}  {name}"
                                for name in sorted(expected - {"SHA256SUMS"})])
        self.assertEqual((first / IMAGE_NAME).read_bytes(), self.image.read_bytes())
        self.check_package(first / PACKAGE_NAME, self.release_id, self.release_public)
        self.check_package(first / BRIDGE_NAME, self.community_id, self.community_public)
        with tarfile.open(first / GEL_NAME, "r:gz") as archive:
            self.assertEqual(archive.extractfile("oos/oos.oosupd").read(), (first / PACKAGE_NAME).read_bytes())
            self.assertEqual(archive.extractfile("oos/oos-update").read(), UPDATER_BYTES)
            self.assertEqual(archive.getmember("oos/oos-update").mode, 0o755)
            self.assertTrue(all(member.mtime == 0 for member in archive.getmembers()))
        manifest = json.loads((first / "release-manifest.json").read_text())
        self.assertEqual((manifest["commit"], manifest["build_id"], manifest["models"]),
                         (COMMIT, BUILD, MODELS.split()))
        artifacts = manifest["artifacts"]
        self.assertEqual(set(artifacts), expected - {"SHA256SUMS", "release-manifest.json"})
        for name, record in artifacts.items():
            self.assertEqual(record["size_bytes"], (first / name).stat().st_size)
        self.assertEqual(artifacts[PACKAGE_NAME]["key_id"], self.release_id)
        self.assertEqual(artifacts[BRIDGE_NAME]["key_id"], self.community_id)
        self.assertEqual(artifacts[BRIDGE_NAME]["trust_anchor"], "SHA256SUMS")

    def test_bridge_key_must_match_and_failure_leaves_no_release(self):
        out = self.root / "wrong-key"
        with self.assertRaisesRegex(SystemExit, "1"), contextlib.redirect_stderr(io.StringIO()) as err:
            self.make_release(out, bridge=True)
        self.assertIn("does not match upstream's community key ID", err.getvalue())
        self.assertFalse(out.exists())

    def test_bridge_with_bad_signature_leaves_no_release(self):
        out = self.root / "bad-signature"
        run_signer = release._run_signer

        def corrupt_bridge(command):
            run_signer(command)
            if command[1] == "pack" and "--untrusted-ok" in command:
                package = command[command.index("--out") + 1]
                data = bytearray(package.read_bytes())
                first_digit = data.index(b"\nsig ") + len(b"\nsig ")
                data[first_digit] = ord("1") if data[first_digit] == ord("0") else ord("0")
                package.write_bytes(data)

        with (patch.object(release, "COMMUNITY_PUBLIC_KEY", self.community_public.read_text().strip()),
              patch.object(release, "_run_signer", side_effect=corrupt_bridge),
              self.assertRaisesRegex(SystemExit, "1"), contextlib.redirect_stderr(io.StringIO()) as err):
            self.make_release(out, bridge=True)
        self.assertIn("SIGNATURE INVALID", err.getvalue())
        self.assertFalse(out.exists())

    def test_bridge_upload_limit_includes_header(self):
        out = self.root / "oversize"
        with (patch.object(release, "COMMUNITY_PUBLIC_KEY", self.community_public.read_text().strip()),
              patch.object(release, "WEB_UPLOAD_LIMIT", self.image.stat().st_size + release.MULTIPART_ALLOWANCE),
              self.assertRaisesRegex(SystemExit, "1"), contextlib.redirect_stderr(io.StringIO()) as err):
            self.make_release(out, bridge=True)
        self.assertIn("exceeds upstream's 70 MiB web upload limit", err.getvalue())
        self.assertFalse(out.exists())

    def test_bridge_upload_limit_reserves_multipart_overhead(self):
        with patch.object(release, "COMMUNITY_PUBLIC_KEY", self.community_public.read_text().strip()):
            sample = self.root / "sample"
            self.make_release(sample, bridge=True)
            body_size = (sample / BRIDGE_NAME).stat().st_size + release.MULTIPART_ALLOWANCE
            with patch.object(release, "WEB_UPLOAD_LIMIT", body_size):
                self.make_release(self.root / "fits", bridge=True)
            out = self.root / "too-large"
            with (patch.object(release, "WEB_UPLOAD_LIMIT", body_size - 1),
                  self.assertRaisesRegex(SystemExit, "1"), contextlib.redirect_stderr(io.StringIO()) as err):
                self.make_release(out, bridge=True)
            self.assertIn("reserving 64 KiB for multipart framing", err.getvalue())
            self.assertFalse(out.exists())

    def test_release_without_bridge_and_existing_destination(self):
        out = self.root / "release"
        self.make_release(out)
        self.assertFalse((out / BRIDGE_NAME).exists())
        sums = (out / "SHA256SUMS").read_bytes()
        with self.assertRaisesRegex(SystemExit, "1"), contextlib.redirect_stderr(io.StringIO()) as err:
            self.make_release(out)
        self.assertIn("must name a new release directory", err.getvalue())
        self.assertEqual((out / "SHA256SUMS").read_bytes(), sums)

    def test_production_signer_refuses_untrusted_release_key(self):
        out = self.root / "untrusted"
        with self.assertRaisesRegex(SystemExit, "1"), contextlib.redirect_stderr(io.StringIO()) as err:
            self.make_release(out, signer=self.updater)
        self.assertIn("signing key is not trusted by this build", err.getvalue())
        self.assertFalse(out.exists())

    def stock_card(self):
        board = release.boot_module().boardconf.load("dho")
        sector = 512
        card = self.root / "card"
        with card.open("wb") as stream:
            stream.truncate((int(board["OOS_OOSDATA_LBA"]) + int(board["OOS_OOSDATA_SECTORS"])) * sector)
            stream.seek(int(board["OOS_PARM_LBA"]) * sector)
            stream.write(b"PARM\0\0\0\0mtdparts=" + board["OOS_PARM_MARKS"].replace(" ", ",").encode() +
                         b",0x00020000@0x0002c000(recovery),0x00038000@0x0004c000(backup)\n")
            stream.seek(int(board["OOS_BOOT_LBA"]) * sector)
            stream.write(b"KRNL\0\0\x10\0")
        return card

    def test_release_package_and_gel_payload_dry_run_on_a_card_file(self):
        out = self.root / "release"
        self.make_release(out)
        card, state = self.stock_card(), self.root / "state"
        before = self.checksum(card)
        opts = ["--card", card, "--keys", self.release_public]
        result = subprocess.run([self.updater, "install", out / PACKAGE_NAME, *opts,
                                 "--state", state, "--model", "DHO804"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("dry run", result.stdout)
        with tarfile.open(out / GEL_NAME, "r:gz") as archive:
            payload = self.root / "gel.oosupd"
            payload.write_bytes(archive.extractfile("oos/oos.oosupd").read())
        result = subprocess.run([self.updater, "gel-install", payload, *opts, "--any-model",
                                 "--vendor", self.root / "no-vendor", "--save", self.root / "rescue"],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("dry run", result.stdout)
        self.assertFalse(state.exists())
        self.assertEqual(self.checksum(card), before)

    @staticmethod
    def checksum(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()
