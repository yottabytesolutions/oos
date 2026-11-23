# SPDX-License-Identifier: GPL-2.0-or-later
"""Compare Rust function bodies before and after a file split.

Run from the repository root: python3 tools/check-rust-split.py BASE path/to/module.rs [...]
Compares each file and its module directory against BASE. Exits 1 for changed, missing or added bodies.
"""

import argparse
from collections import defaultdict
from pathlib import Path
import re
import subprocess


TOKENS = re.compile(
    r'\s+|//[^\n]*|/\*|'
    r'(?:br|cr|r)(?P<hashes>\#*)"(?s:.*?)"(?P=hashes)|'
    r'(?:b|c)?"(?:\\[\s\S]|[^"\\])*"|'
    r"(?:b)?'(?:\\(?:u\{[^}]*\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'|"
    r'(?:r\#)?\w+|->|\S'
)
BLOCK_COMMENT = re.compile(r'/\*|\*/')
DELIMITERS = {'(': ')', '[': ']', '{': '}'}


def tokens(source):
    offset = 0
    while match := TOKENS.match(source, offset):
        token = match[0]
        offset = match.end()
        if token == '/*':
            depth = 1
            while depth:
                mark = BLOCK_COMMENT.search(source, offset)
                if mark is None:
                    raise ValueError('unterminated block comment')
                depth += 1 if mark[0] == '/*' else -1
                offset = mark.end()
        elif not (token.isspace() or token.startswith('//')):
            yield token, match.start()


def body_start(stream, ends, start):
    angles = 0
    while start < len(stream):
        token = stream[start][0]
        if token == '{' and angles == 0:
            return start
        if token == ';':
            return None
        if token == '<':
            angles += 1
        elif token == '>':
            angles -= 1
        start = ends.get(start, start) + 1
    return None


def owners(stream, ends):
    scopes = {}
    for i, (token, _) in enumerate(stream):
        if token not in ('impl', 'trait'):
            continue
        if token == 'impl' and i and stream[i - 1][0] not in ('{', '}', ';', ']', 'unsafe'):
            continue
        start = body_start(stream, ends, i + 1)
        if start is None:
            continue
        header = ' '.join(word for word, _ in stream[i:start])
        scopes[start] = re.sub(r'\b(?:crate|self|super) : : ', '', header)
    return scopes


def function_bodies(path, source):
    stream = list(tokens(source))
    ends, stack = {}, []
    for i, (token, _) in enumerate(stream):
        if token in DELIMITERS:
            stack.append(i)
        elif token in DELIMITERS.values():
            if not stack or DELIMITERS[stream[stack[-1]][0]] != token:
                raise ValueError(f'{path}: unbalanced delimiter {token}')
            ends[stack.pop()] = i
    if stack:
        raise ValueError(f'{path}: unclosed delimiter')
    scopes = owners(stream, ends)
    inside = []
    for i, (token, offset) in enumerate(stream[:-1]):
        if inside and i == inside[-1][0]:
            inside.pop()
        if i in scopes:
            inside.append((ends[i], scopes[i]))
        name = stream[i + 1][0]
        if token != 'fn' or not re.fullmatch(r'(?:r\#)?[^\W\d]\w*', name):
            continue
        start = body_start(stream, ends, i + 2)
        if start is not None:
            line = source.count('\n', 0, offset) + 1
            body = tuple(word for word, _ in stream[start:ends[start] + 1])
            key = (inside[-1][1], name) if inside else ('', name)
            yield key, (body, f'{path}:{line}')


def collect(paths):
    functions = defaultdict(list)
    for path, source in paths:
        for name, entry in function_bodies(path, source):
            functions[name].append(entry)
    return functions


def compare(old, new):
    changed = missing = added = same = 0
    for key in sorted(old.keys() | new.keys()):
        owner, function = key
        name = f'{owner}::{function}' if owner else function
        unmatched = []
        candidates = list(new.get(key, ()))
        for body, site in old.get(key, ()):
            match = next((i for i, (other, _) in enumerate(candidates) if body == other), None)
            if match is None:
                unmatched.append(site)
            else:
                candidates.pop(match)
                same += 1
        for site, (_, other) in zip(unmatched, candidates):
            print(f'changed {name}: {site} -> {other}')
            changed += 1
        for site in unmatched[len(candidates):]:
            print(f'missing {name}: {site}')
            missing += 1
        for _, site in candidates[len(unmatched):]:
            print(f'added {name}: {site}')
            added += 1
    print(f'{same} identical, {changed} changed, {missing} missing, {added} added')
    return bool(changed or missing or added)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('base')
    parser.add_argument('modules', nargs='+', type=Path)
    args = parser.parse_args()
    failed = False
    for module in args.modules:
        tree = module.with_suffix('')
        old_paths = subprocess.check_output(
            ['git', 'ls-tree', '-r', '--name-only', '-z', args.base, '--', str(module), str(tree)], text=True
        ).split('\0')
        old = collect(
            (path, subprocess.check_output(['git', 'show', f'{args.base}:{path}'], text=True))
            for path in old_paths if path.endswith('.rs')
        )
        paths = [module, *sorted(tree.rglob('*.rs'))]
        new = collect((path, path.read_text()) for path in paths if path.is_file())
        print(f'{module}:')
        failed |= compare(old, new)
    return failed


if __name__ == '__main__':
    raise SystemExit(main())
