#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Worktree-local outputs and content-checked build receipts. See OUTPUTS.md."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def target(root=None):
    root = root or ROOT
    path = (root / os.environ.get("CARGO_TARGET_DIR", "target")).resolve()
    if path == root.resolve() or not path.is_relative_to(root.resolve()):
        raise ValueError("CARGO_TARGET_DIR must be strictly inside this worktree")
    if (root / ".git").exists():
        relative = path.relative_to(root.resolve())
        tracked = subprocess.check_output(["git", "-C", str(root), "ls-files", "--", str(relative)])
        if relative.parts[0] == ".git" or tracked:
            raise ValueError("CARGO_TARGET_DIR must not contain Git metadata or tracked sources")
    return path


def digest(path):
    h = hashlib.sha256()
    files = sorted(path.rglob("*")) if path.is_dir() else [path]
    for item in files:
        h.update(os.fsencode(item.relative_to(path) if path.is_dir() else item.name) + b"\0")
        h.update(str(item.lstat().st_mode).encode() + b"\0")
        if item.is_symlink():
            h.update(os.fsencode(os.readlink(item)))
        if item.is_file():
            h.update(item.read_bytes())
    return h.hexdigest()


def revision(root=None):
    root = root or ROOT
    def git(*args):
        return subprocess.check_output(["git", "-C", str(root), *args])
    h = hashlib.sha256()
    # Include new sources as well as tracked edits; generated output is ignored by Git.
    paths = git("ls-files", "-z", "--cached", "--others", "--exclude-standard").split(b"\0")
    out = target(root)
    for name in sorted(set(paths) - {b""}):
        path = root / os.fsdecode(name)
        relative = path.relative_to(root)
        if (path.is_relative_to(out) or relative.parts[0] in ("docs", ".github") or
                path.suffix == ".md" or path.name.startswith("COMMIT_MSG")):
            continue
        # Missing tracked paths are absent inputs, just as after committing their deletion.
        if not path.exists() and not path.is_symlink():
            continue
        h.update(name + b"\0")
        h.update(str(path.lstat().st_mode).encode() + b"\0")
        h.update(os.fsencode(os.readlink(path)) if path.is_symlink() else path.read_bytes())
        h.update(b"\0")
    return h.hexdigest()


# the native rootfs tools: their builder (rootfs/build-<builder>.sh) and their rootfs/deps.lock lines
NATIVE_TOOLS = {
    "e2fsck": ("fsck", "e2fsprogs"),
    "kexec": ("kexec", "kexec-tools"),
    "wpa_supplicant": ("wlan-tools", "wpa_supplicant", "libnl", "libnl-musl-patch"),
    "wpa_cli": ("wlan-tools", "wpa_supplicant", "libnl", "libnl-musl-patch"),
    "nft": ("nft", "libmnl", "libnftnl", "nftables"),
    "dropbearmulti": ("dropbear", "dropbear"),
    "sftp-server": ("sftp-server", "openssh"),
}


def source_identity(path):
    """Native rootfs tools depend only on their builder, dependencies and environment."""
    if path.resolve().parent != (ROOT / "rootfs/out").resolve() or path.name not in NATIVE_TOOLS:
        return revision()
    return hashlib.sha256(dependency_inputs(path.name)).hexdigest()


def receipt(path):
    return path.with_name(path.name + ".oos-build.json")


def compiler_identity():
    identity = hashlib.sha256()
    for program, args in (("rustc", ["-vV"]), ("cargo", ["--version"])):
        result = subprocess.run([program, *args], cwd=ROOT, capture_output=True, check=True, timeout=15)
        identity.update(result.stdout)
        identity.update(b"\0")
    return identity.hexdigest()


def dependency_inputs(name):
    # Use the very same input description as build-dev-image's dep_stale/dep_stamp decision.
    builder, *names = NATIVE_TOOLS[name]
    env = {**os.environ, "DEPS_LOCK": str(ROOT / "rootfs/deps.lock")}
    return subprocess.check_output(
        ["sh", "-eu", "-c", '. "$1"; shift; dep_stamp_text "$@"', "dependency-inputs",
         str(ROOT / "rootfs/deps.sh"), name, str(ROOT / f"rootfs/build-{builder}.sh"), *names],
        cwd=ROOT, env=env)


def metadata(path, tree):
    record = {"tree": tree, "sha256": digest(path)}
    if path.name in {"oos-ui", "oos-fpgad", "oos-update"}:
        record["compiler"] = compiler_identity()
    return record


def stamp(path, before, compiler=None):
    if source_identity(path) != before:
        raise ValueError("source changed during build; rebuild before consumption")
    pending = receipt(path).with_suffix(f".{os.getpid()}.pending")
    record = metadata(path, before)
    if compiler is not None and record.get("compiler") != compiler:
        raise ValueError("compiler changed during build; rebuild before consumption")
    pending.write_text(json.dumps(record) + "\n")
    pending.replace(receipt(path))


def verify(path):
    try:
        record = json.loads(receipt(path).read_text())
    except FileNotFoundError as exc:
        raise ValueError(f"missing build receipt for {path.name}; rebuild with its builder "
                         "(Rust binaries: tools/output.py build)") from exc
    if record != metadata(path, source_identity(path)):
        raise ValueError(f"stale or replaced artifact: {path.name}; rebuild using tools/output.py")
    return record


def main():
    command, *args = sys.argv[1:]
    out = target()
    os.environ["CARGO_TARGET_DIR"] = str(out)
    if command == "target":
        print(out)
    elif command == "revision":
        print(revision())
    elif command == "identity":
        print(source_identity(Path(args[0])))
    elif command == "verify":
        for name in args:
            verify(Path(name))
    elif command == "stamp":
        stamp(Path(args[0]), args[1])
    elif command == "build":
        os.environ.setdefault("CARGO_INCREMENTAL", "0")
        if any(a.startswith(("--target-dir", "--message-format", "--config")) for a in args):
            raise ValueError("select outputs with CARGO_TARGET_DIR; build output overrides are unsupported")
        before = revision()
        compiler = compiler_identity()
        # Stream Cargo diagnostics while collecting only executable artifact paths.
        proc = subprocess.Popen(["cargo", "build", "--locked", "--message-format=json", *args],
                                cwd=ROOT, stdout=subprocess.PIPE, text=True)
        binaries = []
        for line in proc.stdout:
            event = json.loads(line)
            if event.get("executable"):
                binaries.append(Path(event["executable"]))
            if event.get("reason") == "compiler-message":
                print(event["message"].get("rendered", ""), end="", file=sys.stderr)
        if proc.wait():
            raise ValueError("Cargo build failed; no receipts written")
        for binary in binaries:
            stamp(binary, before, compiler)
    else:
        raise ValueError("expected target, revision, identity, build, verify or stamp")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as exc:
        sys.exit(f"output: {exc}")
