//! `cargo xtask ratchet`: writes the ratchet files back after an improvement. Entries only shrink or go; the
//! coverage floors only rise. `--init` writes them from the current state (once, when a rule comes in).

use crate::ratchet::Ratchet;
use crate::{clippy, steps};
use std::path::Path;

const CLIPPY_HEAD: &str = "Clippy and rustc lint findings per crate, file and lint: <count> <crate> <file> <lint>.\n\
Written by `cargo xtask ratchet` (docs/development.md). Counts may only fall.";
const SIZE_HEAD: &str = "Items over the size limits: <size> <kind> <file or crate> <name>.\n\
Written by `cargo xtask ratchet` (docs/development.md). Sizes may only fall.";
const COVERAGE_HEAD: &str = "Line coverage floor per crate in tenths of a percent: <floor> <crate>.\n\
Written by `cargo xtask ratchet` (docs/development.md). Floors may only rise.";

const STATICS_HEAD: &str = "Statics with interior mutability that are not metrics: 1 static <file> <name>.\n\
Written by `cargo xtask ratchet` (docs/development.md). Entries may only go; metrics are listed in architecture.txt.";

const UNSAFE_HEAD: &str = "unsafe outside the allowed paths (architecture.txt): <count> unsafe <file>.\n\
Written by `cargo xtask ratchet` (docs/development.md). Counts may only fall.";

/// The current lint counts, sizes, mutable statics and `unsafe` of the ratcheted crates.
fn current(root: &Path) -> Result<[Ratchet; 4], String> {
    let rules = crate::arch::load(root)?;
    let [mut lints, mut sizes, mut statics, mut unsafes] = [(); 4].map(|_| Ratchet::default());
    for (c, m) in steps::measured(root)?.into_iter().filter(|(c, _)| c.ratchet) {
        let f = clippy::run(root, c.dir, c.target)?;
        if !f.errors.is_empty() {
            return Err(format!("{}: does not build:\n{}", c.dir, f.errors.join("\n")));
        }
        lints.0.extend(f.counts.0);
        unsafes.0.extend(steps::unsafe_now(&m, &rules.unsafe_paths).0);
        sizes.0.extend(m.sizes.0);
        statics.0.extend(m.statics.0.into_iter().filter(|(k, _)| !rules.statics.contains(k)));
    }
    Ok([lints, sizes, statics, unsafes])
}

fn write(root: &Path, name: &str, head: &str, r: &Ratchet) -> Result<(), String> {
    let p = steps::ratchet_path(root, name);
    std::fs::write(&p, r.text(head)).map_err(|e| format!("{}: {}", p.display(), e))?;
    println!("{}: {} entries", p.display(), r.0.len());
    Ok(())
}

/// The coverage floors after `now`: raised to the current value, never lowered.
fn raised(floor: &Ratchet, now: &Ratchet) -> Ratchet {
    Ratchet(now.0.iter().map(|(k, &n)| (k.clone(), n.max(floor.0.get(k).copied().unwrap_or(0)))).collect())
}

/// `--init` only brings in a ratchet file the base does not have yet: over an existing one it would list anything.
fn refuse_init(root: &Path, coverage: bool) -> Result<(), String> {
    let mut names = vec!["clippy.txt", "size.txt", "statics.txt", "unsafe.txt"];
    if coverage {
        names.push("coverage.txt");
    }
    for n in names {
        let rel = format!("xtask/data/{}", n);
        if crate::base::text(root, &rel)?.is_some() {
            return Err(format!("{} exists at the base: --init only introduces a new ratchet file", rel));
        }
    }
    Ok(())
}

/// Tightens (or with `init` writes) the ratchets; `coverage` also measures and raises the floors.
pub fn run(root: &Path, init: bool, coverage: bool) -> Result<(), String> {
    if init {
        refuse_init(root, coverage)?;
    }
    let load = |n: &str| Ratchet::load(&steps::ratchet_path(root, n));
    let files = [
        ("clippy.txt", CLIPPY_HEAD),
        ("size.txt", SIZE_HEAD),
        ("statics.txt", STATICS_HEAD),
        ("unsafe.txt", UNSAFE_HEAD),
    ];
    for ((name, head), now) in files.into_iter().zip(current(root)?) {
        let r = if init { now } else { load(name)?.tightened(&now) };
        write(root, name, head, &r)?;
    }
    if coverage {
        let now = steps::coverage_now(root).map_err(|e| e.join("\n"))?;
        let floor = if init { Ratchet::default() } else { load("coverage.txt")? };
        write(root, "coverage.txt", COVERAGE_HEAD, &raised(&floor, &now))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_only_rise() {
        let floor = Ratchet::parse("900 ui\n800 hal\n").unwrap();
        let now = Ratchet::parse("850 ui\n820 hal\n700 dsp\n").unwrap();
        assert_eq!(raised(&floor, &now), Ratchet::parse("900 ui\n820 hal\n700 dsp\n").unwrap());
    }
}
