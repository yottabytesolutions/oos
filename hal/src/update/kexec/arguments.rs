// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Legacy load skips uncached purgatory hashing to stay inside the hardware watchdog period.

use std::path::Path;

pub(super) fn legacy(dir: &Path, from: u64, cmdline: &str) -> Vec<String> {
    let mut args = vec!["-d".into(), "-c".into(), "-i".into()];
    args.extend([
        format!("--mem-min=0x{from:x}"),
        "-l".into(),
        dir.join("Image").display().to_string(),
        format!("--initrd={}", dir.join("initrd").display()),
        format!("--dtb={}", dir.join("tree.dtb").display()),
        format!("--command-line={cmdline}"),
    ]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_always_skips_the_uncached_digest() {
        assert_eq!(
            legacy(Path::new("/path/to/staged"), 0x40000000, "console=tty0 quiet"),
            [
                "-d",
                "-c",
                "-i",
                "--mem-min=0x40000000",
                "-l",
                "/path/to/staged/Image",
                "--initrd=/path/to/staged/initrd",
                "--dtb=/path/to/staged/tree.dtb",
                "--command-line=console=tty0 quiet",
            ]
        );
    }
}
