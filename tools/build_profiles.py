# SPDX-License-Identifier: GPL-2.0-or-later
"""Read-only selected-profile probes. No pull, install, signing or device access."""
import hashlib
import contextlib
import io
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_env

ROOT = build_env.ROOT

EGL_PROBE = r'''
#include <EGL/egl.h>
#include <GLES2/gl2.h>
int main(void) {
    EGLDisplay d = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    EGLint n, a[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE,
        EGL_OPENGL_ES2_BIT, EGL_NONE}, p[] = {EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE},
        c[] = {EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE};
    EGLConfig f;
    if (!eglInitialize(d, 0, 0) || !eglBindAPI(EGL_OPENGL_ES_API) ||
        !eglChooseConfig(d, a, &f, 1, &n) || !n) return 1;
    EGLSurface s = eglCreatePbufferSurface(d, f, p);
    EGLContext x = eglCreateContext(d, f, EGL_NO_CONTEXT, c);
    if (!eglMakeCurrent(d, s, s, x) || !glGetString(GL_RENDERER)) return 2;
    eglDestroyContext(d, x); eglDestroySurface(d, s); eglTerminate(d);
    return 0;
}
'''


def probe(role, image, profile):
    args = ["run", "--rm", "--pull=never", "--network=none", "--platform", build_env.platform_for(role)]
    script, source = "true", None
    if role == "gpu-test":
        args += ["-i", "-e", "LIBGL_ALWAYS_SOFTWARE=1", "-e", "EGL_PLATFORM=surfaceless"]
        script = "cat > /tmp/probe.c; gcc /tmp/probe.c -lEGL -lGLESv2 -o /tmp/probe; /tmp/probe"
        source = EGL_PROBE
    elif role in ("sim-fw", "sim-ssh"):
        args += ["--cap-add", "NET_ADMIN", "--sysctl", "net.ipv6.conf.all.disable_ipv6=0"]
        script = "ip link set lo up; ip -6 addr show dev lo | grep -q ::1; nft add table inet doctor; nft delete table inet doctor"
        if role == "sim-ssh":
            script += "; command -v ssh; command -v ssh-keygen; command -v sshpass"
    elif role == "kernel":
        script = "aarch64-linux-gnu-gcc --version; make --version; dtc --version; cpio --version; xz --version"
    elif profile == "image":
        script = {"initramfs": "dtc --version; cpio --version; gzip --version",
                  "sim-usb": "test -d /lib/modules; command -v mkfs.vfat; command -v sfdisk",
                  "sim-wlan": "test -d /lib/modules; gcc --version; command -v cpio"}.get(role, "true")
    if role.startswith("rootfs-"):
        script = "gcc --version"
    build_env.docker(*args, image, "sh", "-ec", script, input=source)


def kernel_source():
    text = (ROOT / "kernel/base.sha256").read_text().split()
    source = ROOT / "kernel" / text[1]
    with source.open("rb") as stream:
        actual = hashlib.file_digest(stream, "sha256").hexdigest()
    if actual != text[0]:
        raise ValueError("kernel source checksum mismatch")


def inventory(profile):
    tools = json.loads((ROOT / "tools/build-tools.json").read_text())
    findings = []
    try:
        build_env.docker("info")
    except (ValueError, OSError, subprocess.TimeoutExpired):
        findings.append("docker: daemon unavailable; start Docker")
    for role in tools["profiles"][profile]:
        try:
            image = build_env.resolve(role)
            probe(role, image, profile)
            print(f"ok {role}: identity and disposable probe match")
        except (ValueError, OSError, KeyError, subprocess.TimeoutExpired):
            findings.append(f"{role}: missing/mismatched lock, image or probe; tools/build-env.sh prepare {role}")
    if profile == "image":
        for name, check in [("kernel-source", kernel_source)]:
            try:
                check()
            except (ValueError, OSError, subprocess.SubprocessError):
                findings.append(name + ": unavailable; see kernel/README.md and docs/development.md")
    if profile == "sim":
        try:
            qemu()
        except (ValueError, OSError, subprocess.SubprocessError):
            findings.append("qemu: unavailable; install qemu-system-aarch64; see docs/development.md")
    return findings


def qemu():
    subprocess.run(["qemu-system-aarch64", "--version"], check=True, capture_output=True, timeout=10)


def main():
    if len(sys.argv) not in (2, 3) or sys.argv[1] not in ("gpu", "sim", "image") or (len(sys.argv) == 3 and sys.argv[2] != "--json"):
        return 2
    try:
        with contextlib.redirect_stdout(io.StringIO()) if "--json" in sys.argv else contextlib.nullcontext():
            findings = inventory(sys.argv[1])
    except (ValueError, KeyError, OSError):
        findings = ["profile lock invalid; python3 -I tools/build_env.py validate"]
    if "--json" in sys.argv:
        print(json.dumps({"failed": [line.split(":", 1)[0] for line in findings]}))
    else:
        for line in findings:
            print("FAIL " + line)
    return int(bool(findings))


if __name__ == "__main__":
    sys.exit(main())
