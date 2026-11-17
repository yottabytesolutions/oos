#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Strict RGB PNG comparison and self-contained review artifacts (stdlib only)."""
import html
import json
from pathlib import Path
import struct
import zlib

SIGNATURE = b'\x89PNG\r\n\x1a\n'


def read_png(path):
    data = Path(path).read_bytes()
    if not data.startswith(SIGNATURE):
        raise ValueError('not a PNG')
    pos, payload, header, ended = 8, bytearray(), None, False
    while pos < len(data):
        if len(data) - pos < 12:
            raise ValueError('truncated chunk')
        size, kind = struct.unpack('>I4s', data[pos:pos + 8])
        part = data[pos + 8:pos + 8 + size]
        if pos + size + 12 > len(data):
            raise ValueError('truncated payload')
        crc, = struct.unpack('>I', data[pos + 8 + size:pos + 12 + size])
        if zlib.crc32(kind + part) != crc:
            raise ValueError('bad chunk CRC')
        if header is None and kind != b'IHDR':
            raise ValueError('IHDR must be first')
        if kind == b'IHDR':
            if header is not None or size != 13:
                raise ValueError('invalid IHDR')
            header = struct.unpack('>IIBBBBB', part)
        elif kind == b'IDAT':
            payload.extend(part)
        elif kind == b'IEND':
            if size or pos + 12 != len(data):
                raise ValueError('invalid IEND or trailing data')
            ended = True
            break
        elif not kind[0] & 32:
            raise ValueError('unsupported critical chunk')
        pos += size + 12
    if not ended or header is None:
        raise ValueError('missing IEND')
    w, h, depth, colour, compression, filtering, interlace = header
    if not (0 < w <= 8192 and 0 < h <= 8192) or (depth, colour, compression, filtering, interlace) != (8, 2, 0, 0, 0):
        raise ValueError('requires bounded 8-bit RGB noninterlaced PNG')
    inflater = zlib.decompressobj()
    raw = inflater.decompress(payload, h * (w * 3 + 1) + 1)
    if not inflater.eof or inflater.unused_data or len(raw) != h * (w * 3 + 1):
        raise ValueError('invalid image data length')
    return w, h, unfilter(raw, w, h)


def unfilter(raw, w, h):
    stride, pixels, previous = w * 3, bytearray(), bytearray(w * 3)
    for y in range(h):
        start = y * (stride + 1)
        mode, row = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        if mode > 4:
            raise ValueError('invalid PNG filter')
        if mode:
            for x in range(stride):
                a, b, c = row[x - 3] if x >= 3 else 0, previous[x], previous[x - 3] if x >= 3 else 0
                p = a + b - c
                paeth = min((a, b, c), key=lambda v: abs(p - v))
                row[x] = (row[x] + (0, a, b, (a + b) // 2, paeth)[mode]) & 255
        pixels.extend(row)
        previous = row
    return bytes(pixels)


def write_png(path, image):
    w, h, rgb = image
    if len(rgb) != w * h * 3:
        raise ValueError('RGB length does not match dimensions')
    def chunk(kind, part):
        return struct.pack('>I', len(part)) + kind + part + struct.pack('>I', zlib.crc32(kind + part))
    rows = b''.join(b'\0' + rgb[y * w * 3:(y + 1) * w * 3] for y in range(h))
    Path(path).write_bytes(SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) +
                           chunk(b'IDAT', zlib.compress(rows, 9)) + chunk(b'IEND', b''))


def region_map(w, h, regions):
    labels = bytearray(w * h)
    names = ['remaining']
    for name, x, y, rw, rh in regions:
        if name in names or min(x, y) < 0 or min(rw, rh) <= 0 or x + rw > w or y + rh > h:
            raise ValueError('invalid region')
        names.append(name)
        for row in range(y, y + rh):
            start = row * w + x
            if any(labels[start:start + rw]):
                raise ValueError('overlapping regions')
            labels[start:start + rw] = bytes([len(names) - 1]) * rw
    return labels, names


def compare_rgb(expected, actual, regions=()):
    w, h, a = expected
    if actual[:2] != (w, h) or len(a) != w * h * 3 or len(actual[2]) != len(a):
        raise ValueError('picture size mismatch')
    labels, names = region_map(w, h, regions)
    metrics = {name: dict(pixels=labels.count(i), different=0, over=0, maximum=0, box=None)
               for i, name in enumerate(names)}
    if a != actual[2]:
        for p, (r, g, b) in enumerate(zip(a[0::3], a[1::3], a[2::3])):
            q = p * 3
            error = max(abs(r - actual[2][q]), abs(g - actual[2][q + 1]), abs(b - actual[2][q + 2]))
            if not error:
                continue
            m = metrics[names[labels[p]]]
            m['different'] += 1
            m['over'] += error > 1
            m['maximum'] = max(m['maximum'], error)
            x, y = p % w, p // w
            old = m['box'] or [x, y, x, y]
            m['box'] = [min(old[0], x), min(old[1], y), max(old[2], x), max(old[3], y)]
    for m in metrics.values():
        m['budget'] = m['pixels'] // 1000
        m['pass'] = m['over'] <= m['budget']
    return metrics


def write_diff(path, expected, actual):
    if expected[:2] != actual[:2]:
        raise ValueError('picture size mismatch')
    a, b = expected[2], actual[2]
    diff = bytearray(len(a))
    for q in (() if a == b else range(0, len(a), 3)):
        if a[q:q + 3] != b[q:q + 3]:
            diff[q:q + 3] = b'\xff\x00\xff'
    write_png(path, (*expected[:2], diff))


def write_report(directory, expected, actual, metrics):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    write_png(directory / 'expected.png', expected)
    write_png(directory / 'actual.png', actual)
    write_diff(directory / 'diff.png', expected, actual)
    summary = json.dumps(metrics, indent=2, sort_keys=True)
    (directory / 'metrics.txt').write_text(summary + '\n')
    images = ''.join(f'<figure><figcaption>{n}</figcaption><img width="512" src="{n}.png"></figure>'
                     for n in ('expected', 'actual', 'diff'))
    (directory / 'index.html').write_text('<!doctype html><meta charset="utf-8"><title>Golden review</title>'
                                        '<style>figure{display:inline-block;margin:4px}img{max-width:100%}</style>' +
                                        images + '<pre>' + html.escape(summary) + '</pre>')


def require_captures(directory, names):
    found = {p.name for p in Path(directory).glob('*.png')}
    if found != set(names) or len(names) != len(set(names)):
        raise ValueError(f'capture inventory mismatch: missing={set(names) - found}, extra={found - set(names)}')
    if not names:
        raise ValueError('zero captures selected')
