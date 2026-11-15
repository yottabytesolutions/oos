// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Build-time diagnostic for legacy purgatory only. No runtime environment or same-tree load uses it.

use std::path::Path;

pub(super) const NO_CHECKS: bool = match option_env!("OOS_KEXEC_NO_CHECKS") {
    Some(value) => matches!(value.as_bytes(), [b'1']),
    None => false,
};

pub(super) fn legacy(dir: &Path, from: u64, cmdline: &str, no_checks: bool) -> Vec<String> {
    let mut args = vec!["-d".into(), "-c".into()];
    if no_checks {
        args.push("-i".into());
    }
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
    fn default_arguments_match_the_existing_legacy_load() {
        assert_eq!(
            legacy(Path::new("/path/to/staged"), 0x40000000, "console=tty0 quiet", false),
            [
                "-d",
                "-c",
                "--mem-min=0x40000000",
                "-l",
                "/path/to/staged/Image",
                "--initrd=/path/to/staged/initrd",
                "--dtb=/path/to/staged/tree.dtb",
                "--command-line=console=tty0 quiet",
            ]
        );
    }

    #[test]
    fn diagnostic_adds_only_the_no_checks_option() {
        let dir = Path::new("/path/to/staged");
        let normal = legacy(dir, 0x80000000, "console=tty0", false);
        let mut diagnostic = legacy(dir, 0x80000000, "console=tty0", true);
        assert_eq!(diagnostic.remove(2), "-i");
        assert_eq!(diagnostic, normal);
    }

    #[test]
    fn build_switch_selects_the_legacy_arguments() {
        // The test driver supplies an independent expectation when compiling a diagnostic image's code.
        let expected = std::env::var("OOS_KEXEC_EXPECT_NO_CHECKS").is_ok_and(|value| value == "1");
        assert_eq!(NO_CHECKS, expected);
        let args = legacy(Path::new("/path/to/staged"), 0, "", NO_CHECKS);
        assert_eq!(args.iter().filter(|s| s.as_str() == "-i").count(), usize::from(expected));
    }
}
