//! The repository: its root and the Rust crates the gate covers.

use std::path::{Path, PathBuf};

/// A member of the root workspace; the gate runs its steps crate by crate for counts and reports per crate. A crate
/// without ratchet files allows nothing, except the entries a planned move brings into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crate {
    /// directory under the root, also the crate's name in the ratchet files
    pub dir: &'static str,
    /// built for the scope (aarch64-unknown-linux-musl) and checked for it
    pub target: bool,
}

/// Every crate the gate covers.
pub const CRATES: [Crate; 6] = [
    Crate { dir: "hal", target: true },
    Crate { dir: "dsp", target: true },
    Crate { dir: "core", target: true },
    Crate { dir: "ui", target: true },
    Crate { dir: "fpgad", target: true },
    Crate { dir: "xtask", target: false },
];

/// The scope's target triple.
pub const TARGET: &str = "aarch64-unknown-linux-musl";

/// The repository root: the parent of xtask's manifest directory.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

/// Where the gate keeps its data files (ratchets, rules).
pub fn data(root: &Path) -> PathBuf {
    root.join("xtask").join("data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_holds_the_crates() {
        let r = root();
        assert!(CRATES.iter().all(|c| r.join(c.dir).join("Cargo.toml").exists()));
        assert!(CRATES.iter().any(|c| c.dir == "xtask" && !c.target));
    }
}
