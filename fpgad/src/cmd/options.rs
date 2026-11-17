// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

pub(crate) fn value<'a>(args: &'a [String], flag: &str) -> Option<Option<&'a str>> {
    args.iter().position(|arg| arg == flag).map(|i| args.get(i + 1).map(String::as_str))
}

#[cfg(target_os = "linux")]
pub(super) fn num(s: &str) -> Result<u32, String> {
    match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16),
        None => s.parse(),
    }
    .map_err(|e| format!("{}: {}", s, e))
}

#[cfg(test)]
mod tests {
    use super::value;

    #[test]
    fn distinguishes_absent_options_from_missing_values() {
        let args = ["--freq"].map(String::from);
        assert_eq!(value(&args, "--missing"), None);
        assert_eq!(value(&args, "--freq"), Some(None));
        assert_eq!(value(&[], "--freq"), None);
    }

    #[test]
    fn takes_the_first_exact_flag_and_its_immediate_value() {
        let args = ["--frequency", "100", "--freq", "200", "--freq", "300"].map(String::from);
        assert_eq!(value(&args, "--freq"), Some(Some("200")));
        assert_eq!(value(&args, "--fre"), None);
    }

    #[test]
    fn preserves_empty_negative_and_option_shaped_values() {
        let args = ["--empty", "", "--at", "-2000", "--freq", "--dry"].map(String::from);
        assert_eq!(value(&args, "--empty"), Some(Some("")));
        assert_eq!(value(&args, "--at"), Some(Some("-2000")));
        assert_eq!(value(&args, "--freq"), Some(Some("--dry")));
    }
}
