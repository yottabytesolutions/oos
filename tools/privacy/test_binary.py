# SPDX-License-Identifier: GPL-2.0-or-later
"""Binary contents never become text; PNG metadata and filenames remain scanned."""

import struct
import tempfile
import unittest
import zlib
from pathlib import Path
from unittest.mock import patch

import git_input
import scan

EMAIL = b"synthetic@" + b"mail.invalid"
SIGNATURE = b"\x89PNG\r\n\x1a\n"


def chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def png(*metadata: bytes) -> bytes:
    # Stored DEFLATE blocks leave the email-like pixel bytes visible in IDAT.
    pixels = EMAIL + b" " * (-len(EMAIL) % 3)
    header = struct.pack(">IIBBBBB", len(pixels) // 3, 1, 8, 2, 0, 0, 0)
    return (SIGNATURE + chunk(b"IHDR", header) + b"".join(metadata)
            + chunk(b"IDAT", zlib.compress(b"\0" + pixels, level=0)) + chunk(b"IEND", b""))


class BinaryTests(unittest.TestCase):
    def findings(self, data: bytes, name: str = "fixture.png") -> list[tuple[str, int, str]]:
        with patch.object(git_input, "entries", return_value=iter([(name, "new")])), \
                patch.object(git_input, "git", return_value=data):
            return list(scan.findings(git_input.changed(Path("."), {}, None), set(), []))

    def test_png_pixel_data_with_email_like_bytes_passes(self) -> None:
        data = png()
        self.assertIn(EMAIL, data)
        self.assertEqual(self.findings(data), [])
        self.assertEqual(self.findings(data, "renamed.dat"), [])

    def test_png_text_email_fails(self) -> None:
        self.assertEqual({rule for _, _, rule in self.findings(png(chunk(b"tEXt", b"Note\0" + EMAIL)))},
                         {"email"})

    def test_png_altered_text_chunk_type_fails_as_input_error(self) -> None:
        metadata = chunk(b"tEXt", b"Note\0" + EMAIL)
        altered = metadata[:4] + b"xEXt" + metadata[8:]
        with self.assertRaisesRegex(ValueError, "invalid PNG chunk CRC"):
            self.findings(png(altered))

    def test_png_invalid_crc_fails_for_every_chunk_type(self) -> None:
        for kind, payload in ((b"IHDR", b"header"), (b"tEXt", b"Note\0safe"),
                              (b"IDAT", b"pixels"), (b"vpAg", b"unknown"), (b"IEND", b"")):
            with self.subTest(kind=kind):
                valid = chunk(kind, payload)
                corrupt = valid[:-1] + bytes([valid[-1] ^ 1])
                with self.assertRaisesRegex(ValueError, "invalid PNG chunk CRC"):
                    self.findings(png(corrupt))

    def test_png_valid_unknown_chunk_passes(self) -> None:
        self.assertEqual(self.findings(png(chunk(b"vpAg", b"unknown"))), [])

    def test_png_compressed_and_international_text_email_fails(self) -> None:
        for kind, payload in ((b"zTXt", b"Note\0\0" + zlib.compress(EMAIL)),
                              (b"iTXt", b"Note\0\0\0en\0Title\0" + EMAIL),
                              (b"iTXt", b"Note\0\1\0en\0Title\0" + zlib.compress(EMAIL))):
            with self.subTest(kind=kind, compressed=payload[5]):
                self.assertEqual({rule for _, _, rule in self.findings(png(chunk(kind, payload)))}, {"email"})

    def test_png_metadata_keywords_and_translated_keywords_are_scanned(self) -> None:
        for kind, payload in ((b"tEXt", EMAIL + b"\0safe"),
                              (b"zTXt", EMAIL + b"\0\0" + zlib.compress(b"safe")),
                              (b"iTXt", b"Note\0\0\0en\0" + EMAIL + b"\0safe")):
            with self.subTest(kind=kind):
                self.assertEqual({rule for _, _, rule in self.findings(png(chunk(kind, payload)))}, {"email"})

    def test_other_binary_contents_are_skipped_but_names_are_scanned(self) -> None:
        name = EMAIL.decode() + ".bin"
        for data in (b"\0" + EMAIL, b"\xff\xfe" + EMAIL, png()):
            with self.subTest(prefix=data[:4]):
                self.assertEqual(self.findings(data, "fixture.bin"), [])
                self.assertEqual(self.findings(data, name), [(name, 0, "email")])
        self.assertEqual(self.findings(EMAIL), [("fixture.png", 1, "email")])

    def test_working_tree_uses_the_same_content_rules(self) -> None:
        for data, expected in ((png(), set()), (png(chunk(b"tEXt", b"Note\0" + EMAIL)), {"email"}),
                               (b"\0" + EMAIL, set())):
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "fixture").write_bytes(data)
                with patch.object(scan, "git", return_value=b"fixture\0"):
                    found = list(scan.findings(scan.working_tree(root), set(), []))
                self.assertEqual({rule for _, _, rule in found}, expected)
                name = EMAIL.decode() + ".png"
                (root / name).write_bytes(data)
                with patch.object(scan, "git", return_value=name.encode() + b"\0"):
                    found = list(scan.findings(scan.working_tree(root), set(), []))
                self.assertIn((name, 0, "email"), found)

    def test_changed_png_compares_only_added_metadata_lines(self) -> None:
        old = png(chunk(b"tEXt", b"Note\0" + EMAIL))
        new = png(chunk(b"tEXt", b"Note\0" + EMAIL + b"\nsafe"))
        with patch.object(git_input, "git", side_effect=lambda root, *args: {"old": old, "new": new}[args[-1]]):
            self.assertEqual(list(git_input.blob_additions(Path("."), "old", "new")), [(3, "safe")])
        new = png(chunk(b"tEXt", b"Note\0safe\n" + EMAIL))
        with patch.object(git_input, "git", side_effect=lambda root, *args: {"old": png(), "new": new}[args[-1]]):
            found = list(scan.findings((("fixture", n, line, False)
                                       for n, line in git_input.blob_additions(Path("."), "old", "new")), set(), []))
        self.assertEqual(found, [("fixture", 3, "email")])

    def test_malformed_png_text_fails_as_input_error(self) -> None:
        for data in (SIGNATURE + b"\0", png(chunk(b"zTXt", b"Note\0\0invalid")),
                     png(chunk(b"iTXt", b"Note\0\2\0en\0Title\0text"))):
            with self.subTest(data=data[:12]), self.assertRaises(ValueError):
                self.findings(data)


if __name__ == "__main__":
    unittest.main()
