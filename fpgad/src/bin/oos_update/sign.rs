// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Host signing commands and trusted-key loading for oos-update.

use crate::{die, put, read_file, Args, STDOUT_CLOSED};
use oos_hal::update::{self, card, hash::hex, kexec, pkg};
use std::path::{Path, PathBuf};

/// The built-in keys; `--keys FILE` adds keys for `verify` only (and for tests on a card file, never on the unit's card).
pub(crate) fn keys(a: &Args, extra: bool) -> Vec<[u8; 32]> {
    let mut k = update::trusted_keys();
    if !extra && a.val("--keys").is_some() {
        die("--keys is for verify (and card-file tests) only");
    }
    if let Some(f) = a.val("--keys") {
        let t = std::fs::read_to_string(f).unwrap_or_else(|e| die(&format!("{}: {}", f, e)));
        k.extend(pkg::parse_keys(&t));
    }
    k
}

fn load_seed(path: &Path) -> [u8; 32] {
    let t = std::fs::read_to_string(path)
        .unwrap_or_else(|e| die(&format!("{}: {} (oos-update keygen)", path.display(), e)));
    let seed = parse_seed(&t).unwrap_or_else(|| die(&format!("{}: no 64-digit hex secret key", path.display())));
    if update::trusted_keys().contains(&seed) {
        die("selected signing seed is a trusted public key; use its secret seed instead");
    }
    seed
}

fn parse_seed(t: &str) -> Option<[u8; 32]> {
    t.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .find(|l| !l.is_empty())
        .and_then(update::hash::unhex)
        .and_then(|b| b.try_into().ok())
}

fn signing_seed(a: &Args) -> [u8; 32] {
    let key = a
        .val("--key")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("OOS_SIGNING_KEY").filter(|v| !v.is_empty()).map(PathBuf::from));
    let key = key.unwrap_or_else(|| die("signing requires --key FILE or OOS_SIGNING_KEY"));
    load_seed(&key)
}

pub(crate) fn keygen(a: &Args) {
    let out =
        a.val("--out").map(PathBuf::from).unwrap_or_else(|| die("keygen needs --out FILE outside the repository"));
    if out.exists() {
        die(&format!(
            "{} exists: not overwritten (a lost key means every unit needs a hand-installed image)",
            out.display()
        ));
    }
    let mut seed = [0u8; 32];
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut seed))
            .unwrap_or_else(|e| die(&e.to_string()));
    }
    let pk = oos_hal::update::ed25519::public_key(&seed);
    if let Some(d) = out.parent() {
        std::fs::create_dir_all(d).unwrap_or_else(|e| die(&e.to_string()));
    }
    let text = format!(
        "# OOS update signing key (Ed25519 secret seed). Keep it secret and backed up; never put it in the repository.\n# public key {} (id {})\n{}\n",
        hex(&pk),
        hex(&pkg::key_id(&pk)),
        hex(&seed)
    );
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        let mut f = options.open(&out).unwrap_or_else(|e| die(&format!("{}: {}", out.display(), e)));
        f.write_all(text.as_bytes()).unwrap_or_else(|e| die(&e.to_string()));
    }
    out!("secret key: {} (mode 0600)", out.display());
    out!("add this line to oos/keys/update.pub:");
    out!("{}  # id {}", hex(&pk), hex(&pkg::key_id(&pk)));
}

pub(crate) fn pack(a: &Args) {
    let seed = signing_seed(a);
    let pk = oos_hal::update::ed25519::public_key(&seed);
    if !update::trusted_keys().contains(&pk) {
        if !a.flag("--untrusted-ok") {
            die("signing key is not trusted by this build; use --untrusted-ok to sign anyway");
        }
        eprintln!(
            "oos-update: WARNING: key {} is not in oos/keys/update.pub: units will refuse this package",
            hex(&pkg::key_id(&pk))
        );
    }
    let img = read_file(a.val("--image").unwrap_or_else(|| die("pack --image IMG --build ID --out FILE")));
    let build = a.val("--build").unwrap_or_else(|| die("pack needs --build ID (the image's /etc/oos-build)"));
    let out = a.val("--out").unwrap_or_else(|| die("pack needs --out FILE"));
    let kernel = a
        .val("--kernel")
        .map(str::to_string)
        .unwrap_or_else(|| build.split(" kernel ").nth(1).unwrap_or("?").to_string());
    let info = card::check_image(&img).unwrap_or_else(|e| die(&format!("image: {}", e)));
    if info.len != img.len() {
        die(&format!("image is {} bytes, its sections end at {}", img.len(), info.len));
    }
    let models = update::models::image(&img).unwrap_or_else(|e| die(&format!("image models: {}", e)));
    if let Some(selected) = a.val("--models") {
        if selected != models {
            die(&format!("--models '{}', image models '{}': refused", selected, models));
        }
    }
    let capabilities = [("models", models.as_str()), kexec::LINE, kexec::STAGED_LINE];
    let p = pkg::build_with(&seed, build, &kernel, &img, &capabilities).unwrap_or_else(|e| die(&e));
    pkg::verify(&p, &[pk]).unwrap_or_else(|e| die(&format!("self-check: {}", e)));
    std::fs::write(out, &p).unwrap_or_else(|e| die(&format!("{}: {}", out, e)));
    out!("{}: build {}, {} bytes, key {}", out, build, p.len(), hex(&pkg::key_id(&pk)));
}
