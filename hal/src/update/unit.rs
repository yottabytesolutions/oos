// SPDX-License-Identifier: GPL-2.0-or-later

//! Build identity, independent of the device tree: a kernel update may bring a new tree for the same unit.

use super::{hash::hex, pkg, short, InstallOpt, Report};
use std::path::Path;

pub const FILE: &str = "/etc/oos-unit";
pub const NONE: &str = "none";

impl Default for InstallOpt<'_> {
    fn default() -> Self {
        Self { running_unit: NONE, execute: false, downgrade: false, force: false, any_unit: false }
    }
}

/// A unit is a build profile name, never a serial number or a file path.
pub fn validate(name: &str) -> Result<&str, String> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(
            "unit must be 1 to 64 ASCII letters, digits, dots, underscores or hyphens, starting with a letter or digit"
                .into(),
        );
    }
    Ok(name)
}

/// Missing identity in an older image means `none`; unreadable or malformed identity refuses the update.
pub fn read(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => validate(t.trim()).map(str::to_string),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NONE.into()),
        Err(e) => Err(format!("reading the running image's unit: {}", e)),
    }
}

pub fn running() -> Result<String, String> {
    read(Path::new(FILE))
}

/// Keep the signed extension bytes untouched, including when an old package has no unit line.
pub fn package(m: &pkg::Manifest) -> Result<&str, String> {
    let mut values = m.extra.iter().filter(|(k, _)| k == "unit").map(|(_, v)| v.as_str());
    let name = values.next().unwrap_or(NONE);
    if values.next().is_some() {
        return Err("package has duplicate unit lines".into());
    }
    validate(name)
}

/// None for a match; an explicit, printable warning when a mismatch was overridden.
pub fn check(m: &pkg::Manifest, running: &str, any_unit: bool) -> Result<Option<String>, String> {
    let running = validate(running)?;
    let unit = package(m)?;
    if unit == running {
        return Ok(None);
    }
    let mismatch = format!("package unit '{}', running image unit '{}'", unit, running);
    if !any_unit {
        return Err(format!("{}: refused (give --any-unit to override)", mismatch));
    }
    Ok(Some(format!("WARNING: --any-unit override: {}", mismatch)))
}

pub(super) fn checked_package<'a>(
    package: &'a [u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
) -> Result<(pkg::Header, &'a [u8], Report), String> {
    let (h, img) = pkg::verify(package, keys)?;
    let m = &h.manifest;
    let warning = check(m, opt.running_unit, opt.any_unit)?;
    let mut r = Report::default();
    r.say(format!(
        "package: build {}, kernel {}, {} bytes, signature ok (key {}), unit {}",
        short(&m.build),
        m.kernel,
        m.size,
        hex(&m.key),
        self::package(m)?
    ));
    if let Some(w) = warning {
        r.say(w);
    }
    Ok((h, img, r))
}

/// The profile embedded in the image's initramfs, never inferred from a signing argument or host identity.
pub fn image(img: &[u8]) -> Result<String, String> {
    let (_, ramdisk, _) = super::kexec::parts(img)?;
    // An image without a ramdisk has no embedded identity.
    if ramdisk.is_empty() {
        return Ok(NONE.into());
    }
    let rd = super::inflate::gunzip(ramdisk, 512 << 20)?;
    let Some(bytes) = super::inflate::cpio_file(&rd, "etc/oos-unit") else { return Ok(NONE.into()) };
    let text = std::str::from_utf8(bytes).map_err(|_| "image unit is not UTF-8")?;
    validate(text.trim()).map(str::to_string)
}
