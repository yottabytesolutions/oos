// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! gzip (RFC 1952) and deflate (RFC 1951) unpacking, for reading a file out of an image's initramfs: the build of the
//! image on the card (`image_build`), which the install from a USB stick compares with its own.

use miniz_oxide::inflate::{
    core::{decompress, inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF, DecompressorOxide},
    TINFLStatus,
};

/// Raw deflate data, at most `limit` bytes out. Also the number of input bytes it took.
pub fn inflate(b: &[u8], limit: usize) -> Result<(Vec<u8>, usize), String> {
    let mut decoder = Box::<DecompressorOxide>::default();
    let mut out = vec![0; b.len().saturating_mul(2).max(1).min(limit)];
    let (mut read, mut written) = (0, 0);
    loop {
        let (status, consumed, produced) =
            decompress(&mut decoder, &b[read..], &mut out, written, TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF);
        read += consumed;
        written += produced;
        match status {
            TINFLStatus::Done => {
                out.truncate(written);
                return Ok((out, read));
            }
            TINFLStatus::HasMoreOutput if out.len() < limit => {
                out.resize(out.len().saturating_mul(2).min(limit), 0);
            }
            TINFLStatus::HasMoreOutput => return Err("deflate: more data than allowed".into()),
            TINFLStatus::FailedCannotMakeProgress | TINFLStatus::NeedsMoreInput => {
                return Err("deflate: data cut short".into());
            }
            _ => return Err("deflate: invalid data".into()),
        }
    }
}

/// A gzip file (one member), at most `limit` bytes out.
pub fn gunzip(b: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    if b.len() < 18 || b[0] != 0x1f || b[1] != 0x8b || b[2] != 8 {
        return Err("not gzip".into());
    }
    let flags = b[3];
    let mut at = 10;
    if flags & 4 != 0 {
        let x = u16::from_le_bytes([b[at], b[at + 1]]) as usize;
        at += 2 + x;
    }
    for f in [8u8, 16] {
        if flags & f != 0 {
            at += b.get(at..).ok_or("gzip cut short")?.iter().position(|&c| c == 0).ok_or("gzip cut short")? + 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    let (out, _) = inflate(b.get(at..).ok_or("gzip cut short")?, limit)?;
    Ok(out)
}

/// A file of a newc cpio archive (with or without a leading "./").
pub fn cpio_file<'a>(mut b: &'a [u8], name: &str) -> Option<&'a [u8]> {
    while b.len() >= 110 && &b[..6] == b"070701" {
        let f = |i: usize| {
            std::str::from_utf8(&b[6 + i * 8..14 + i * 8]).ok().and_then(|s| u32::from_str_radix(s, 16).ok())
        };
        let (size, nlen) = (f(6)? as usize, f(11)? as usize);
        let n = b.get(110..110 + nlen.saturating_sub(1))?;
        let at = (110 + nlen).div_ceil(4) * 4;
        let data = b.get(at..at + size)?;
        if n == b"TRAILER!!!" {
            return None;
        }
        if n == name.as_bytes() || n.strip_prefix(b"./") == Some(name.as_bytes()) {
            return Some(data);
        }
        b = b.get((at + size).div_ceil(4) * 4..)?;
    }
    None
}

/// The build of an OOS image (its initramfs' /etc/oos-build, as `pack --build` names it), when it can be read.
pub fn image_build(img: &[u8]) -> Option<String> {
    let (_, ramdisk, _) = super::kexec::parts(img).ok()?;
    let rd = gunzip(ramdisk, 512 << 20).ok()?;
    let b = cpio_file(&rd, "etc/oos-build")?;
    Some(String::from_utf8_lossy(b).trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// gzip with stored blocks (what a test can make without a compressor).
    pub fn gzip_stored(data: &[u8]) -> Vec<u8> {
        let mut b = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
        let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[][..]] } else { data.chunks(65535).collect() };
        for (i, c) in chunks.iter().enumerate() {
            b.push((i + 1 == chunks.len()) as u8);
            b.extend_from_slice(&(c.len() as u16).to_le_bytes());
            b.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
            b.extend_from_slice(c);
        }
        b.extend_from_slice(&[0; 8]);
        b
    }

    #[test]
    fn stored_blocks() {
        let d: Vec<u8> = (0..200_000u32).map(|i| (i * 7) as u8).collect();
        assert_eq!(gunzip(&gzip_stored(&d), 1 << 20).unwrap(), d);
        assert!(gunzip(&gzip_stored(&d), 1000).is_err());
    }

    #[test]
    fn raw_stream_boundaries_and_limits() {
        for data in [&b""[..], &b"a"[..], &b"hello"[..]] {
            let gzip = gzip_stored(data);
            let raw = &gzip[10..];
            let (out, consumed) = inflate(raw, data.len()).unwrap();
            assert_eq!(out, data);
            assert_eq!(consumed, raw.len() - 8, "leave the gzip trailer unread");
            assert_eq!(inflate(&raw[..consumed], data.len()).unwrap(), (out, consumed));
            assert!(inflate(&raw[..consumed - 1], data.len()).is_err());
            if !data.is_empty() {
                assert!(inflate(raw, data.len() - 1).is_err());
            }
        }
        assert!(inflate(&[], 10).is_err());
        assert!(inflate(&[7], 10).is_err(), "reserved block type");
        let fixed = [203, 72, 205, 201, 201, 87, 200, 64, 39, 185, 0];
        assert_eq!(inflate(&fixed, 24).unwrap(), (b"hello hello hello hello\n".to_vec(), fixed.len()));
        assert!(inflate(&fixed, 23).is_err());
        assert_eq!(inflate(&[3, 0], 0).unwrap(), (Vec::new(), 2));
    }

    #[test]
    fn fixed_and_dynamic_codes() {
        // Python's gzip.compress(..., 9, mtime=0): a short text (fixed codes) and a longer one (dynamic codes)
        let fixed = [
            31, 139, 8, 0, 0, 0, 0, 0, 2, 255, 203, 72, 205, 201, 201, 87, 200, 64, 39, 185, 0, 0, 136, 89, 11, 24, 0,
            0, 0,
        ];
        assert_eq!(gunzip(&fixed, 1 << 20).unwrap(), b"hello hello hello hello\n");
        let dynamic = [
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0xff, 0x95, 0xd5, 0x39, 0x0e, 0xc2, 0x50, 0x0c, 0x04,
            0xd0, 0x9e, 0x53, 0xf8, 0x08, 0x78, 0x63, 0x3b, 0x0e, 0x28, 0x88, 0x40, 0x48, 0xd8, 0xa2, 0x70, 0x7c, 0x84,
            0x68, 0x52, 0x20, 0xdb, 0x53, 0x7b, 0x7e, 0x31, 0x7a, 0xf2, 0x77, 0xd7, 0xf6, 0x0d, 0x2d, 0x77, 0xf4, 0x3a,
            0x35, 0x74, 0x1f, 0xdb, 0xc3, 0x85, 0xf6, 0x8f, 0x61, 0xea, 0xe9, 0x38, 0xbc, 0x69, 0xd1, 0x7d, 0x87, 0xfc,
            0x7f, 0x78, 0x1e, 0xaf, 0xb7, 0xe7, 0x2f, 0x21, 0x41, 0x62, 0x16, 0xd3, 0x2c, 0x36, 0xcb, 0x5a, 0x29, 0x3b,
            0x7b, 0xe0, 0x51, 0x85, 0x55, 0x5a, 0x61, 0x5d, 0xab, 0xb0, 0x01, 0x2a, 0x6c, 0xd1, 0x0a, 0x1c, 0x33, 0xe4,
            0x0e, 0x5c, 0x84, 0x60, 0x44, 0x82, 0x61, 0x0a, 0x0e, 0x2d, 0x38, 0xc7, 0xe0, 0xa2, 0x06, 0x23, 0x1c, 0x0c,
            0x7b, 0x48, 0xe8, 0x21, 0x85, 0xbd, 0x28, 0x7a, 0x08, 0xe2, 0x21, 0xb0, 0x87, 0x84, 0x1e, 0x92, 0x7b, 0x48,
            0xd1, 0x43, 0x10, 0x0f, 0x81, 0x3d, 0x34, 0xf4, 0xd0, 0xdc, 0x43, 0xab, 0x1f, 0x15, 0xe2, 0xa1, 0xb0, 0x87,
            0x86, 0x1e, 0x9a, 0x7b, 0x68, 0xd1, 0x43, 0x11, 0x0f, 0x85, 0x3d, 0x2c, 0xf4, 0xb0, 0xdc, 0xc3, 0x8a, 0x1e,
            0x06, 0x5d, 0x0e, 0xd8, 0xc3, 0x42, 0x0f, 0xcb, 0x3d, 0xac, 0xe8, 0x61, 0x88, 0x87, 0xc1, 0x1e, 0x1e, 0x7a,
            0x78, 0xee, 0xe1, 0x45, 0x0f, 0x47, 0x3c, 0x1c, 0x3f, 0xe5, 0xa1, 0x87, 0xe7, 0x1e, 0x5e, 0xf4, 0x70, 0xc4,
            0xc3, 0x11, 0x8f, 0x0f, 0xea, 0x0f, 0xcf, 0x53, 0x56, 0x09, 0x00, 0x00,
        ];
        let text: String =
            (0..60).map(|i| format!("line {}: the quick brown fox {}\n", i, "jumps".repeat(i % 5))).collect();
        assert_eq!(gunzip(&dynamic, 1 << 20).unwrap(), text.as_bytes());
        assert!(gunzip(&dynamic, 100).is_err(), "the limit holds");
    }

    #[test]
    fn a_file_of_an_archive() {
        let a = crate::update::kexec::newc(&[
            ("etc/oos-build", 0o100644, b"20261112-121847 kernel 6.12.115+\n"),
            ("bin/x", 0o100755, b""),
        ]);
        assert_eq!(cpio_file(&a, "etc/oos-build"), Some(&b"20261112-121847 kernel 6.12.115+\n"[..]));
        assert_eq!(cpio_file(&a, "etc/other"), None);
    }

    /// An image's build: OOS_IMG set to a boot image (target/image/oos-dev.img).
    #[test]
    #[ignore = "private image: set OOS_IMG"]
    fn the_build_of_an_image() {
        let img = std::fs::read(std::env::var("OOS_IMG").unwrap()).unwrap();
        let t = std::time::Instant::now();
        let b = image_build(&img).expect("a build");
        println!("{} ({} ms)", b, t.elapsed().as_millis());
    }
}
