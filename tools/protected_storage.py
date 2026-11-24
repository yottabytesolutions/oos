# SPDX-License-Identifier: GPL-2.0-or-later
"""Reject raw storage-write primitives outside the ADR 0011 checked writer."""
import ast
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import script_syntax

WRITER = 'hal/src/update/card/writer.rs'
DEVICE = r'/dev/(?:block/)?(?:mmcblk|[r]?disk|mtd|fram|by-name/)[^\s"\']*'
# These dd destinations are regular host images, never hardware devices.
IMAGE_OUTPUTS = {
    'tools/sdcard-clone.sh': {'"$out"', '$p', '"/out/$OUT"'},
    'tools/gel-sim.sh': {'/w/card'},
}


def violations(name, text):
    if '/tests/' in name or Path(name).name.startswith('test_'):
        return []
    bad = []
    if name.endswith('.py'):
        tree = ast.parse(text)
        for node in ast.walk(tree):
            if not isinstance(node, ast.Call):
                continue
            func = ast.unparse(node.func)
            if func not in {'open', 'os.open'} and not func.endswith('.open'):
                continue
            first_mode = 1 if func in {'open', 'os.open'} else 0
            modes = [*node.args[first_mode:first_mode + 1], *(k.value for k in node.keywords if k.arg in {'mode', 'flags'})]
            mode = ' '.join(ast.unparse(m) for m in modes)
            target = (ast.unparse(node.args[0]) if node.args else func) if first_mode else func
            if 'O_NOCTTY' in mode:
                continue
            update = any(isinstance(n, ast.Constant) and isinstance(n.value, str)
                         and n.value.startswith('r') and '+' in n.value for m in modes for n in ast.walk(m))
            if update or ('O_RDWR' in mode or 'O_WRONLY' in mode) or (
                re.search(r'[wax+]', mode) and re.search(DEVICE + r'|\b(target|device|card|disk|partition|flash|fram)\b', target)
            ):
                bad.append(f'{node.lineno}: raw writable storage open')
    elif name.endswith('.rs'):
        # Unit-test setup may synthesize arbitrary card contents.
        production = text.split('#[cfg(test)]\nmod tests', 1)[0]
        if name != WRITER:
            for statement in production.split(';'):
                if re.search(r'\.write\s*\(\s*(true|write|writable)\s*\)', statement) and (
                    re.search(DEVICE, statement) or (name.startswith('hal/src/update/card') and '.create_new(true)' not in statement)
                ):
                    bad.append('raw writable storage open')
            if re.search(r'\b(pwrite|pwrite64)\s*\(|self\.f\.write', production):
                bad.append('raw storage write')
            if re.search(r'(?:fs::write|File::create|fs::copy)\s*\(\s*"' + DEVICE, production):
                bad.append('raw storage output')
    else:
        for number, line in enumerate(text.splitlines(), 1):
            if line.lstrip().startswith(('#', '//', '*')):
                continue
            if re.search(r'\bdd\b', line):
                for target in re.findall(r'\bof=("[^"]*"|\S+)', line):
                    if target not in IMAGE_OUTPUTS.get(name, set()):
                        bad.append(f'{number}: raw dd output')
            if re.search(r'(?:>|\btee\s+|\bcp\s+\S+\s+)(?:\s*"?)' + DEVICE, line):
                bad.append(f'{number}: raw storage output')
            if re.search(r'>\s*"?\$(?:dev|DEV|device|card|disk)\b', line):
                bad.append(f'{number}: raw storage redirection')
            if re.search(r'\b(flash_erase|flashcp|nandwrite|mtd_debug)\b', line):
                bad.append(f'{number}: raw flash command')
            if re.search(r'\b(mke2fs|e2fsck|tune2fs|mkfs(?:\.\w+)?)\s+.*(?:\$(?:P|dev|DEV)\b|' + DEVICE + ')', line):
                bad.append(f'{number}: filesystem tool on raw storage')
            if re.search(r'open\s*\(\s*"' + DEVICE + r'".*O_(WRONLY|RDWR)', line):
                bad.append(f'{number}: raw C storage open')
    # The existing SPI monitor sends BOOT only; no erase/program transport exists.
    if name.endswith(('.rs', '.c')) and 'spidev' in text and re.search(r'["\'][^"\'\n]*(?:PROG|ERASE)', text):
        bad.append('FPGA flash programming outside checked writer')
    return [f'{name}: {message}' for message in bad]


def check(root):
    bad = []
    for name in script_syntax.tracked(root):
        path = root / name
        if not path.is_file():
            continue
        with path.open('rb') as stream:
            script = stream.read(2) == b'#!'
        if path.suffix in {'.rs', '.py', '.sh', '.c', '.h'} or script:
            bad.extend(violations(name, path.read_text()))
    if bad:
        raise ValueError('\n'.join(bad))
    print('protected-storage: raw writes use the ADR 0011 checked writer')


if __name__ == '__main__':
    try:
        check(Path(__file__).resolve().parents[1])
    except (ValueError, OSError, SyntaxError) as exc:
        sys.exit(f'protected-storage: {exc}')
