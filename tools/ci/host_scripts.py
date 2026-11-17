#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""The shell scripts that run on a developer's host, checked with shellcheck as POSIX sh at warning level, so they
keep working on Linux, the BSDs and macOS.

  host_scripts.py list    print them
  host_scripts.py check   run shellcheck --shell=sh --severity=warning on them (pr.yml, fast job)

A host script is every tracked sh script and sh fragment (tools/script_syntax.py classifies them) outside SCOPE:
those run on the scope (busybox or the stock Android shell) or only inside a Linux container, where GNU and busybox
forms are fine.
"""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import script_syntax  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
# Prefixes of scripts that never run on the host.
SCOPE = ('rootfs/initramfs/', 'rootfs/board/', 'rootfs/out/', 'boot/gel/', 'tools/platform-probe/', 'kernel/tools/',
         'boot/unit-hook-image.sh', 'boot/unit-hook-dtb.sh')


def host_scripts(root):
    names = []
    for name in script_syntax.tracked(root):
        path = root / name
        if name.startswith(SCOPE) or not path.is_file():
            continue
        if script_syntax.interpreter(path, name) == 'sh':
            names.append(name)
    if not names:
        raise ValueError('no host shell scripts found')
    return names


def check(root, names):
    shellcheck = shutil.which('shellcheck')
    if shellcheck is None:
        raise ValueError('shellcheck is not installed')
    result = subprocess.run([shellcheck, '--shell=sh', '--severity=warning', '--format=gcc', *names], cwd=root,
                            check=False)
    if result.returncode != 0:
        raise ValueError(f'shellcheck found problems in host scripts (exit {result.returncode})')
    print(f'host scripts: {len(names)} pass shellcheck --shell=sh')


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('command', choices=('list', 'check'))
    args = parser.parse_args()
    names = host_scripts(ROOT)
    if args.command == 'list':
        print('\n'.join(names))
    else:
        check(ROOT, names)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError) as exc:
        sys.exit(f'host_scripts: {exc}')
