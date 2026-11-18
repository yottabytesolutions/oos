# SPDX-License-Identifier: GPL-2.0-or-later
"""Extract scannable text by content, without decoding arbitrary binary bytes."""

import zlib

PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def metadata_text(kind: bytes, payload: bytes) -> str:
    keyword, rest = payload.split(b"\0", 1)
    fields = [keyword.decode("latin-1")]
    if kind == b"tEXt":
        fields.append(rest.decode("latin-1"))
    elif kind == b"zTXt":
        if rest[:1] != b"\0":
            raise ValueError("invalid PNG text compression method")
        fields.append(zlib.decompress(rest[1:]).decode("latin-1"))
    else:
        if len(rest) < 2 or rest[0] not in (0, 1) or rest[1] != 0:
            raise ValueError("invalid PNG international text compression")
        language, translated, text = rest[2:].split(b"\0", 2)
        fields.extend((language.decode("ascii"), translated.decode("utf-8")))
        fields.append((zlib.decompress(text) if rest[0] else text).decode("utf-8"))
    return "\n".join(fields)


def png_text(data: bytes) -> str:
    offset = len(PNG_SIGNATURE)
    parts = []
    while offset + 12 <= len(data):
        size = int.from_bytes(data[offset:offset + 4], "big")
        kind = data[offset + 4:offset + 8]
        end = offset + 12 + size
        if end > len(data):
            raise ValueError("truncated PNG chunk")
        stored_crc = int.from_bytes(data[end - 4:end], "big")
        if zlib.crc32(data[offset + 4:end - 4]) != stored_crc:
            raise ValueError("invalid PNG chunk CRC")
        if kind in (b"tEXt", b"iTXt", b"zTXt"):
            try:
                parts.append(metadata_text(kind, data[offset + 8:end - 4]))
            except zlib.error as exc:
                raise ValueError("invalid compressed PNG text") from exc
        offset = end
        if kind == b"IEND":
            return "\n".join(parts)
    raise ValueError("missing PNG end chunk")


def content_text(data: bytes) -> str | None:
    if data.startswith(PNG_SIGNATURE):
        return png_text(data)
    if b"\0" in data:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None
