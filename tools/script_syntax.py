"""Deterministic Git script discovery; compile without running Python or shell bodies."""
import os
from pathlib import Path
import py_compile
import re
import shlex
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import output

# These files are sourced by /bin/sh and intentionally have no shebang.
FRAGMENTS = {'tools/output.sh', 'boot/unit-hook.sh', 'boot/unit-hook-dtb.sh', 'boot/unit-hook-image.sh', 'rootfs/deps.sh',
             'rootfs/board/dho/etc/oos-board', 'rootfs/out/unit-hook/image.sh',
             'rootfs/out/unit-hook/dtb.sh'}
SHELLS = {'sh', 'bash', 'dash', 'mksh', 'ksh', 'zsh'}


def tracked(root):
    raw = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root)
    if raw and not raw.endswith(b'\0'):
        raise ValueError('Git returned a non-NUL script inventory')
    names = sorted(set(os.fsdecode(n) for n in raw.split(b'\0') if n))
    if not names:
        raise ValueError('empty Git discovery; restore tracked scripts')
    return names


def interpreter(path, name):
    data = path.read_bytes()
    first = data.split(b'\n', 1)[0]
    words = shlex.split(first[2:].decode()) if first.startswith(b'#!') else []
    if words[:2] == ['/bin/busybox', 'sh']:
        return 'sh'
    if words[:1] == ['/system/bin/sh']:
        return 'mksh'
    if words:
        program = Path(words.pop(0)).name
        if program == 'env':
            if words[:1] == ['-S']:
                words.pop(0)
            if not words or words[0].startswith('-'):
                raise ValueError(f'{name}: unsupported env shebang')
            program = words.pop(0)
        if re.fullmatch(r'python3(?:\.\d+)?', program):
            return 'python'
        if program in SHELLS:
            return program
        raise ValueError(f'{name}: unclassified executable script interpreter')
    if path.suffix == '.py':
        return 'python'
    if name in FRAGMENTS:
        return 'sh'
    if path.suffix == '.sh':
        raise ValueError(f'{name}: shell fragment needs an explicit interpreter mapping')
    if name == 'kernel/tools/git-log-answers.txt':
        return None
    if path.suffix in {'.c', '.h', '.dts', '.dtsi'} or path.name in {'Makefile'}:
        return None
    if path.stat().st_mode & 0o111 and not data.startswith((b'\x7fELF', b'MZ')):
        raise ValueError(f'{name}: unclassified executable script')
    return None


def check(root, target):
    counts = {'python': 0, 'shell': 0}
    for name in tracked(root):
        path = root / name
        if not path.is_file():
            # Deleted files are absent inputs. New executable symlinks need a deliberate classification.
            if path.is_symlink():
                raise ValueError(f'{name}: dangling script input')
            continue
        kind = interpreter(path, name)
        if kind == 'python':
            dest = target / 'script-bytecode' / (name + 'c')
            dest.parent.mkdir(parents=True, exist_ok=True)
            py_compile.compile(str(path), cfile=str(dest), doraise=True)
            counts['python'] += 1
        elif kind:
            subprocess.run([kind, '-n', str(path)], check=True, cwd=root)
            counts['shell'] += 1
    if not all(counts.values()):
        raise ValueError('empty Python or shell selection; restore script inputs')
    print(f"script-syntax: {counts['python']} Python, {counts['shell']} shell scripts")


if __name__ == '__main__':
    try:
        check(output.ROOT, output.target())
    except (ValueError, OSError, py_compile.PyCompileError, subprocess.SubprocessError) as exc:
        sys.exit(f'script-syntax: {exc}; install the declared shell/Python or repair the script')
