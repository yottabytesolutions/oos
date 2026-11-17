# SPDX-License-Identifier: GPL-2.0-or-later
"""Base and recipe identities with informational package provenance. Inventory never pulls; preparation is explicit."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "tools/container-images.json"
LABEL = "org.oos.environment"


def digest(value):
    return isinstance(value, str) and re.fullmatch(r"sha256:[a-f0-9]{64}", value) is not None


class Prerequisite(ValueError):
    """A diagnostic composed only from repository names and static instructions."""


def base_for(role):
    recipe = ROOT / "tools/build-env" / (role + ".Dockerfile")
    match = re.fullmatch(r"FROM ((?:ubuntu|alpine):[0-9.]+@sha256:[a-f0-9]{64})", recipe.read_text().splitlines()[0])
    if not match:
        raise Prerequisite(f"{role}: recipe must start with a digest-pinned base")
    return match[1]


def load():
    lock = json.loads(LOCK.read_text())
    if lock["schema"] != 1:
        raise Prerequisite("container lock: invalid schema")
    for role, entry in lock["roles"].items():
        if not re.fullmatch(r"[a-z][a-z0-9-]*", role):
            raise Prerequisite("container lock: invalid role name")
        recipe = ROOT / "tools/build-env" / (role + ".Dockerfile")
        if hashlib.sha256(recipe.read_bytes()).hexdigest() != entry["recipe_sha256"]:
            raise Prerequisite(f"{role}: recipe hash changed; review and update the lock")
        index = base_for(role).split("@", 1)[1]
        manifests = lock["manifests"][index]
        if set(manifests) != {"linux/arm64", "linux/amd64"} or any(not digest(m) for m in manifests.values()):
            raise Prerequisite(f"{role}: invalid base platform manifests")
        if not entry["requested"]:
            raise Prerequisite(f"{role}: invalid package request")
        extra = entry.get("platform_requested", {})
        if any(p not in ("linux/arm64", "linux/amd64") for p in extra):
            raise Prerequisite(f"{role}: invalid package request platform")
        for requested in [entry["requested"], *extra.values()]:
            if not isinstance(requested, list) or any(
                    not isinstance(p, str) or not re.fullmatch(r"[a-zA-Z0-9+_.-]+", p) for p in requested):
                raise Prerequisite(f"{role}: invalid package request")
    return lock


def platform_for(role, platform=None):
    if platform:
        return platform
    if role in ("kernel", "gpu-test"):
        # Docker daemon architecture, which may differ from the CLI host architecture.
        arch = docker("info", "--format", "{{.Architecture}}")
        if arch not in ("aarch64", "arm64", "x86_64", "amd64"):
            raise Prerequisite(f"{role}: unsupported Docker daemon architecture")
        return "linux/arm64" if arch in ("aarch64", "arm64") else "linux/amd64"
    return "linux/arm64"


def identity(role, platform=None):
    lock = load()
    entry = lock["roles"][role]
    platform = platform_for(role, platform)
    if platform not in ("linux/arm64", "linux/amd64"):
        raise Prerequisite(f"{role}: unsupported base platform")
    requested = entry["requested"] + entry.get("platform_requested", {}).get(platform, [])
    name, index = base_for(role).split("@", 1)
    base = name + "@" + lock["manifests"][index][platform]
    contract = {"base": base, "platform": platform,
                "recipe": entry["recipe_sha256"], "requested": requested}
    key = hashlib.sha256(json.dumps(contract, sort_keys=True).encode()).hexdigest()
    return contract, "oos-env-" + role + ":" + key, key


def docker(*args, input=None):
    result = subprocess.run(["docker", *args], input=input, capture_output=True, text=True, timeout=1800 if args[0] == "build" else 60)
    if result.returncode:
        # Docker output can contain host paths, environment values or registry credentials.
        raise Prerequisite("Docker command failed; verify the daemon, requested package availability and selected role setup (docs/development.md)")
    return result.stdout.strip()


def actual_packages(role, image, platform):
    alpine = base_for(role).startswith("alpine:")
    command = "apk info -v | sort" if alpine else "dpkg-query -W -f '${Package}=${Version}\\n' | sort"
    text = docker("run", "--rm", "--pull=never", "--network=none", "--platform", platform,
                  image, "sh", "-ec", command)
    if alpine:
        # apk info -v separates a name/version at the last '-<digit>'.
        return sorted(re.sub(r"^(.+)-(\d[^ ]*-r\d+)$", r"\1=\2", l) for l in text.splitlines())
    return sorted(text.splitlines())


def resolve(role, platform=None):
    contract, tag, key = identity(role, platform)
    info = json.loads(docker("image", "inspect", tag))[0]
    base = json.loads(docker("image", "inspect", contract["base"]))[0]
    layers = base.get("RootFS", {}).get("Layers", [])
    derived = info.get("RootFS", {}).get("Layers", [])
    manifest = contract["base"].split("@", 1)[1]
    if (not layers or derived[:len(layers)] != layers
            or not any(ref.endswith("@" + manifest) for ref in base.get("RepoDigests", []))
            or base.get("Os") + "/" + base.get("Architecture") != contract["platform"]):
        raise Prerequisite(f"{role}: cached base digest/layer identity mismatch; tools/build-env.sh prepare {role}")
    if (info.get("Os") + "/" + info.get("Architecture") != contract["platform"]
            or info.get("Config", {}).get("Labels", {}).get(LABEL) != key
            or not digest(info.get("Id"))):
        raise Prerequisite(f"{role}: cached image identity mismatch; tools/build-env.sh prepare {role}")
    # Use the immutable local ID after verification, never the mutable alias.
    image = info["Id"]
    return image


def prepare(role, platform=None):
    contract, tag, key = identity(role, platform)
    try:
        return resolve(role, contract["platform"])
    except (ValueError, subprocess.TimeoutExpired):
        pass
    packages = " ".join(contract["requested"])
    recipe = (ROOT / "tools/build-env" / (role + ".Dockerfile")).read_text()
    recipe = recipe.replace(base_for(role), contract["base"], 1)
    recipe = recipe.replace("@PACKAGES@", packages)
    recipe += f'\nLABEL {LABEL}="{key}"\n'
    docker("pull", "--platform", contract["platform"], contract["base"])
    docker("build", "--platform", contract["platform"], "--pull", "-t", tag, "-", input=recipe)
    return resolve(role, contract["platform"])


def provenance(role, image, platform, output):
    """Record the actual immutable image's packages, without comparing against a lock."""
    packages = actual_packages(role, image, platform)
    output = Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=output.parent, delete=False) as stream:
        pending = Path(stream.name)
        try:
            stream.write("\n".join(packages) + "\n")
            stream.close()
            pending.replace(output)
        finally:
            pending.unlink(missing_ok=True)


def output_probe():
    result = subprocess.run([sys.executable, "-I", str(ROOT / "tools/output.py"), "target"],
                            capture_output=True, text=True)
    if result.returncode:
        raise Prerequisite("output selection rejected; choose an isolated worktree-local directory")
    target = Path(result.stdout.strip())
    target.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="doctor-", dir=target) as scratch:
        (Path(scratch) / "write-probe").write_bytes(b"probe")


def workspace():
    # Isolate mutable Docker build volumes without publishing the worktree path.
    return hashlib.sha256(str(ROOT).encode()).hexdigest()[:16]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("validate", "resolve", "prepare", "identity", "output", "provenance", "platform", "workspace"))
    parser.add_argument("role", nargs="?")
    parser.add_argument("--platform", choices=("linux/arm64", "linux/amd64"))
    parser.add_argument("--image")
    parser.add_argument("--output")
    args = parser.parse_args()
    try:
        if args.command == "validate":
            load()
            rust = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]
            if set(rust) != {"channel", "profile", "components", "targets"}:
                raise Prerequisite("rust-toolchain.toml: unexpected keys")
        elif args.command == "workspace":
            print(workspace())
        elif args.command == "output":
            output_probe()
        else:
            if args.role not in load()["roles"]:
                parser.error("select a role from tools/container-images.json")
            if args.command == "provenance":
                if not digest(args.image) or not args.output:
                    parser.error("provenance requires --image sha256:ID and --output FILE")
                provenance(args.role, args.image, platform_for(args.role, args.platform), args.output)
            elif args.command == "platform":
                print(platform_for(args.role, args.platform))
            elif args.command == "identity":
                print(identity(args.role, args.platform)[2])
            else:
                print((resolve if args.command == "resolve" else prepare)(args.role, args.platform))
        return 0
    except Prerequisite as error:
        print(str(error), file=sys.stderr)
        return 1
    except (ValueError, KeyError, IndexError, TypeError, OSError, subprocess.TimeoutExpired):
        # Do not echo exceptions with filesystem paths or raw subprocess output.
        print("build environment prerequisite failed: validate the role's base digest, recipe, platform and package requests; see docs/development.md", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
