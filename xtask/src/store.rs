//! Where the ratchets live: one file per crate and kind, `xtask/data/<crate>/<kind>.txt`, so branches that touch
//! different crates do not touch the same file. A base that predates the split has one file per kind
//! (`xtask/data/<kind>.txt`); its entries of the crate are the crate's base then.

use crate::base::{self, At};
use crate::ratchet::Ratchet;
use std::path::{Path, PathBuf};

/// A kind of ratchet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Clippy,
    Size,
    Statics,
    Unsafe,
    Allow,
    Known,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Clippy, Kind::Size, Kind::Statics, Kind::Unsafe, Kind::Allow, Kind::Known];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Clippy => "clippy",
            Kind::Size => "size",
            Kind::Statics => "statics",
            Kind::Unsafe => "unsafe",
            Kind::Allow => "allow",
            Kind::Known => "known",
        }
    }

    /// The header comment of the file.
    pub fn head(self) -> &'static str {
        match self {
            Kind::Clippy => "Clippy and rustc lint findings per file and lint: <count> <crate> <file> <lint>.",
            Kind::Size => "Items over the size limits: <size> <kind> <file or crate> <name>.",
            Kind::Statics => "Statics with interior mutability that are not metrics: 1 static <file> <name>.",
            Kind::Unsafe => "unsafe outside the allowed paths (architecture.txt): <count> unsafe <file>.",
            Kind::Allow => "#[allow] or #[expect] of clippy::disallowed_methods: <count> allow <file> <lint>.",
            Kind::Known => "Known failures, #[ignore = \"known: task NNNN\"]: 1 known <file> <test> <task>.",
        }
    }
}

/// The file of `kind` for `krate`, relative to the root.
pub fn rel(krate: &str, kind: Kind) -> String {
    format!("xtask/data/{}/{}.txt", krate, kind.name())
}

fn path(root: &Path, krate: &str, kind: Kind) -> PathBuf {
    root.join(rel(krate, kind))
}

/// The ratchet of `kind` for `krate` in the working tree (missing: empty).
pub fn load(root: &Path, krate: &str, kind: Kind) -> Result<Ratchet, String> {
    Ratchet::load(&path(root, krate, kind))
}

/// Whether the ratchet file of `kind` for `krate` exists in the working tree.
pub fn exists(root: &Path, krate: &str, kind: Kind) -> bool {
    path(root, krate, kind).exists()
}

/// Writes the ratchet of `kind` for `krate`.
pub fn save(root: &Path, krate: &str, kind: Kind, r: &Ratchet) -> Result<(), String> {
    let p = path(root, krate, kind);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {}", d.display(), e))?;
    }
    let head =
        format!("{}\nWritten by `cargo xtask ratchet` (docs/development.md). Values may only fall.", kind.head());
    std::fs::write(&p, r.text(&head)).map_err(|e| format!("{}: {}", p.display(), e))
}

/// The ratchet of `kind` for `krate` at the base `commit`. None when the base has no gate (nothing to compare);
/// empty when the gate exists but the file does not (a new crate or kind starts clean).
pub fn at_base(
    ctx: &crate::run::Context,
    root: &Path,
    commit: &str,
    krate: &str,
    kind: Kind,
) -> Result<Option<Ratchet>, String> {
    let split = base::at(ctx, root, commit, &rel(krate, kind))?;
    let text = match split {
        At::Present(t) => return Ratchet::parse(&t).map(Some),
        At::NoGate => return Ok(None),
        At::Absent => base::at(ctx, root, commit, &format!("xtask/data/{}.txt", kind.name()))?,
    };
    match text {
        At::Present(t) => Ok(Some(of_crate(&Ratchet::parse(&t)?, krate))),
        _ => Ok(Some(Ratchet::default())),
    }
}

/// Entries of `r` that belong to `krate`: the second word is the crate or a path in it, or the first word is the
/// crate (clippy keys).
pub fn of_crate(r: &Ratchet, krate: &str) -> Ratchet {
    let prefix = format!("{}/", krate);
    let mine = |k: &str| {
        k.split(' ').nth(1).is_some_and(|w| w == krate || w.starts_with(&prefix))
            || k.starts_with(&format!("{} ", krate))
    };
    Ratchet(r.0.iter().filter(|(k, _)| mine(k)).map(|(k, v)| (k.clone(), *v)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_split_by_crate() {
        let r = Ratchet::parse("3 file ui/src/a.rs\n2 impls ui Ui\n9 ui ui/src/a.rs clippy::x\n4 file hal/src/b.rs\n")
            .expect("ratchet");
        assert_eq!(of_crate(&r, "ui").0.len(), 3);
        assert_eq!(of_crate(&r, "hal").0.len(), 1);
        assert!(of_crate(&r, "u").0.is_empty());
    }
}
