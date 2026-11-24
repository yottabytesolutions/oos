// SPDX-License-Identifier: GPL-2.0-or-later

//! Supported image models, checked against hardware independently of the device tree.

use super::{hash::hex, pkg, short, InstallOpt, Report};

pub const NONE: &str = "none";

impl Default for InstallOpt<'_> {
    fn default() -> Self {
        Self { running_model: NONE, execute: false, downgrade: false, force: false, any_model: false }
    }
}

/// A model name contains no whitespace or path separators.
pub fn validate(name: &str) -> Result<&str, String> {
    if name.is_empty()
        || name.len() > 64
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    {
        return Err(
            "model must be 1 to 64 ASCII letters, digits, dots, underscores or hyphens, starting with a letter or digit"
                .into(),
        );
    }
    Ok(name)
}

/// Detect hardware identity without image profiles or experimental model overrides.
pub fn running() -> Result<String, String> {
    running_from(std::path::Path::new("/rigol/data/vendor.bin"))
}

pub(super) fn running_from(path: &std::path::Path) -> Result<String, String> {
    let vendor = std::fs::read(path).map_err(|e| format!("reading device model: {}", e))?;
    let p = crate::model::Platform::build(Some(&vendor), None, None);
    if p.source != crate::model::Source::Vendor || p.foreign.is_some() {
        return Err("device model could not be detected".into());
    }
    Ok(p.caps.name)
}

/// A signed list of supported models, independent of any device identity.
fn validate_models(names: &str) -> Result<&str, String> {
    if names.is_empty() {
        return Err("model must be a nonempty list".into());
    }
    for name in names.split(' ') {
        validate(name)?;
    }
    Ok(names)
}

/// Keep the signed extension bytes untouched, including when an old package has no model list.
pub fn package(m: &pkg::Manifest) -> Result<&str, String> {
    let mut values = m.extra.iter().filter(|(k, _)| k == "models").map(|(_, v)| v.as_str());
    let name = values.next().unwrap_or(NONE);
    if values.next().is_some() {
        return Err("package has duplicate model lists".into());
    }
    validate_models(name)
}

/// None for a match; an explicit, printable warning when a mismatch was overridden.
pub fn check(m: &pkg::Manifest, running: &str, any_model: bool) -> Result<Option<String>, String> {
    let running = validate(running)?;
    let models = package(m)?;
    if models.split(' ').any(|name| name == running) {
        return Ok(None);
    }
    let mismatch = format!("package models '{}', running device model '{}'", models, running);
    if !any_model {
        return Err(format!("{}: refused (give --any-model to override)", mismatch));
    }
    Ok(Some(format!("WARNING: --any-model override: {}", mismatch)))
}

pub(super) fn checked_package<'a>(
    package: &'a [u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
) -> Result<(pkg::Header, &'a [u8], Report), String> {
    let (h, img) = pkg::verify(package, keys)?;
    let m = &h.manifest;
    let warning = check(m, opt.running_model, opt.any_model)?;
    let mut r = Report::default();
    r.say(format!(
        "package: build {}, kernel {}, {} bytes, signature ok (key {}), models {}",
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

/// The model list embedded in the image's initramfs, never inferred from a signing argument or host identity.
pub fn image(img: &[u8]) -> Result<String, String> {
    let (_, ramdisk, _) = super::kexec::parts(img)?;
    // An image without a ramdisk has no embedded identity.
    if ramdisk.is_empty() {
        return Ok(NONE.into());
    }
    let rd = super::inflate::gunzip(ramdisk, 512 << 20)?;
    initramfs_models(&rd)
}

/// Read the supported model list from an unpacked newc initramfs.
pub fn initramfs_models(initramfs: &[u8]) -> Result<String, String> {
    let Some(bytes) = super::inflate::cpio_file(initramfs, "etc/oos-models") else { return Ok(NONE.into()) };
    let text = std::str::from_utf8(bytes).map_err(|_| "device model is not UTF-8")?;
    validate_models(text.trim()).map(str::to_string)
}
