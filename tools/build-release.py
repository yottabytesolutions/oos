#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Assemble an offline release from an already built DHO image."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from image_validate import ROOT, boot_module, cpio

# Upstream v0.1.15 keys/update.pub, community key (ID 1d64e92f58d378be).
COMMUNITY_PUBLIC_KEY = "f73d56d8b05852c2fbbb2bcc79ac03420d885df19df3c5405e0958485191ffcf"
# The upstream web limit applies to the entire multipart upload body.
WEB_UPLOAD_LIMIT = 70 << 20
MULTIPART_ALLOWANCE = 64 << 10
BRIDGE_LABEL = "bridge-for-upstream-verify-with-SHA256SUMS"


def _external_key(path):
    if path is None:
        raise ValueError("signing requires --key FILE or OOS_SIGNING_KEY")
    resolved = path.resolve()
    if path.absolute().is_relative_to(ROOT) or resolved.is_relative_to(ROOT):
        raise ValueError("signing keys must be outside the repository")
    if not resolved.is_file():
        raise ValueError("signing key must be a regular file")
    return resolved


def _checkout_git(*args):
    # Bind Git to this checkout even inside a parent repository or a Git hook.
    environment = {name: value for name, value in os.environ.items() if not name.startswith("GIT_")}
    command = ["git", "--no-optional-locks", "--git-dir=.git", "--work-tree=.", *args]
    result = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True)
    if result.returncode:
        raise ValueError(f"cannot inspect release checkout with git {args[0]}")
    return result.stdout


def _require_clean_checkout(commit):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("--commit must be a full Git commit hash")
    head = _checkout_git("rev-parse", "--verify", "HEAD").strip().decode("ascii")
    if commit != head:
        raise ValueError(f"--commit must match checkout HEAD ({head})")
    if _checkout_git("status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignore-submodules=none"):
        raise ValueError("release checkout must be clean (no tracked changes or non-ignored untracked files)")


def _image_inputs(image, boot):
    try:
        fields, kernel, ramdisk, second = boot.parts(image.read_bytes())
    except (ZeroDivisionError, struct.error) as exc:
        raise ValueError("image has an invalid boot header") from exc
    if fields[7] != boot.PAGE or tuple(map(len, (kernel, ramdisk, second))) != tuple(fields[0:6:2]):
        raise ValueError("image has invalid boot section bounds")
    entries = cpio(gzip.decompress(ramdisk))

    def regular(name):
        mode, data = entries.get(name, (0, b""))
        if mode & 0o170000 != 0o100000 or not data:
            raise ValueError(f"image is missing regular file {name}")
        return mode, data

    build = regular("etc/oos-build")[1].decode("ascii").strip()
    models = regular("etc/oos-models")[1].decode("ascii").strip()
    mode, updater = regular("bin/oos-update")
    if not re.fullmatch(r"[0-9]{8}-[0-9]{6} kernel [A-Za-z0-9_.+\-]+", build):
        raise ValueError("image has an invalid release build ID")
    if not models or models == "none":
        raise ValueError("image has no release models")
    if not mode & 0o111:
        raise ValueError("image updater is not executable")
    return build, models, updater


def _header_key(package, build, models):
    with package.open("rb") as source:
        header, separator, _ = source.read(4096).partition(b"\n\n")
    if not separator or not header.startswith(b"OOS-UPDATE 1\n"):
        raise ValueError("updater produced an invalid package header")
    lines = [line.split(" ", 1) for line in header.decode("ascii").splitlines()[1:]]
    fields = dict(lines)
    if fields.get("build") != build or fields.get("models") != models:
        raise ValueError("signed package metadata differs from the image")
    # A package can announce more than one trial start: `trial kexec` and `trial staged`.
    if ["trial", "kexec"] not in lines:
        raise ValueError("signed package is missing 'trial kexec'")
    key_id = fields.get("key", "")
    if not re.fullmatch(r"[0-9a-f]{16}", key_id):
        raise ValueError("signed package is missing a key ID")
    return key_id


def _run_signer(command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode:
        raise ValueError(result.stderr.strip() or f"oos-update {command[1]} exited {result.returncode}")


def _verify_bridge(signer, package, key_id, size):
    expected_id = hashlib.sha256(bytes.fromhex(COMMUNITY_PUBLIC_KEY)).hexdigest()[:16]
    if key_id != expected_id:
        raise ValueError("bridge key does not match upstream's community key ID")
    if size > WEB_UPLOAD_LIMIT - MULTIPART_ALLOWANCE:
        raise ValueError("bridge package exceeds upstream's 70 MiB web upload limit "
                         "after reserving 64 KiB for multipart framing")
    with tempfile.NamedTemporaryFile(mode="w", encoding="ascii", prefix="oos-bridge-public-") as keys:
        keys.write(COMMUNITY_PUBLIC_KEY + "\n")
        keys.flush()
        _run_signer([signer, "verify", package, "--keys", keys.name])


def _pack(signer, image, package, build, models, key, *, community=False):
    command = [signer, "pack", "--image", image, "--build", build, "--models", models,
               "--key", key, "--out", package]
    if community:
        command.append("--untrusted-ok")
    _run_signer(command)
    key_id = _header_key(package, build, models)
    size = package.stat().st_size
    if community:
        _verify_bridge(signer, package, key_id, size)
    else:
        _run_signer([signer, "verify", package])
    return {"size_bytes": size, "key_id": key_id}


def _finalize(directory, tag, commit, build, models, artifacts):
    manifest = {"tag": tag, "commit": commit, "build_id": build,
                "models": models.split(), "artifacts": artifacts}
    path = directory / "release-manifest.json"
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    lines = []
    for path in sorted(directory.iterdir()):
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        lines.append(f"{digest}  {path.name}\n")
    (directory / "SHA256SUMS").write_text("".join(lines), encoding="ascii")


def build_release(args):
    key = _external_key(args.key)
    community_key = _external_key(args.community_key) if args.community_key else None
    if community_key and (community_key == key or
                          community_key.parts[-2:] != ("keys", "community-signing.key")):
        raise ValueError("--community-key must point to upstream's keys/community-signing.key")
    if not re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", args.tag):
        raise ValueError("--tag must be vX.Y.Z")
    _require_clean_checkout(args.commit)
    destination = args.out
    if destination.exists() or destination.is_symlink():
        raise ValueError("--out must name a new release directory")
    boot = boot_module()
    from gel_package import write_gel

    if not args.image.is_file() or not 0 < args.image.stat().st_size <= boot.RECOVERY_BYTES:
        raise ValueError("--image must be a regular boot image that fits in recovery")
    signer = args.updater.resolve(strict=True)
    stem = boot.boardconf.load("dho")["OOS_GEL_STEM"]
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".oos-release-", dir=destination.parent) as temp:
        stage = Path(temp) / "release"
        stage.mkdir()
        image = stage / f"oos-{args.tag}.img"
        shutil.copyfile(args.image, image)
        build, models, updater = _image_inputs(image, boot)
        package = image.with_suffix(".oosupd")
        artifacts = {image.name: {"size_bytes": image.stat().st_size}}
        artifacts[package.name] = _pack(signer, image, package, build, models, key)
        gel = stage / f"{stem}_Install.GEL"
        with gel.open("wb") as output:
            write_gel(output, package, updater, stem)
        artifacts[gel.name] = {"size_bytes": gel.stat().st_size}
        if community_key:
            bridge = stage / f"oos-{args.tag}-{BRIDGE_LABEL}.oosupd"
            record = _pack(signer, image, bridge, build, models, community_key, community=True)
            record.update(trust_anchor="SHA256SUMS")
            artifacts[bridge.name] = record
        _finalize(stage, args.tag, args.commit, build, models, artifacts)
        stage.rename(destination)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", required=True, type=Path, help="unsigned release image")
    parser.add_argument("--tag", required=True, help="release tag, vX.Y.Z")
    parser.add_argument("--commit", required=True, help="image's full Git commit hash, matching checkout HEAD")
    parser.add_argument("--updater", required=True, type=Path, help="host oos-update from the release commit")
    parser.add_argument("--key", type=Path, help="project signing key outside the repository, or OOS_SIGNING_KEY")
    parser.add_argument("--community-key", type=Path, help="optional upstream keys/community-signing.key for bridge")
    parser.add_argument("--out", required=True, type=Path, help="new directory for release artifacts")
    args = parser.parse_args(argv)
    if args.key is None and (value := os.environ.get("OOS_SIGNING_KEY")):
        args.key = Path(value)
    try:
        build_release(args)
    except (ValueError, OSError, EOFError) as exc:
        parser.exit(1, f"build-release: {exc}\n")
    print(f"Release written to {args.out}")


if __name__ == "__main__":
    main()
