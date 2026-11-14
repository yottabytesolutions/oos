//! Size limits: files, functions, structs and the spread of a type's impl blocks. Only test code is exempt.

mod statics;
pub mod unsafety;
mod visit;

use crate::ratchet::Ratchet;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Lines of a source file outside `#[cfg(test)]` items.
pub const FILE_LINES: u64 = 400;
/// Lines of a function, from `fn` to its closing brace.
pub const FN_LINES: u64 = 60;
/// Fields of a struct.
pub const STRUCT_FIELDS: u64 = 12;
/// Files with inherent impl blocks of one type in a crate.
pub const IMPL_FILES: u64 = 3;

/// The Rust files under `dir`, sorted.
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Adds `key` with `value` when it is over `limit`; a key seen twice in a file gets `#2`, `#3`.
fn over(now: &mut Ratchet, key: String, value: u64, limit: u64) {
    if value <= limit {
        return;
    }
    let mut k = key.clone();
    let mut n = 2;
    while now.0.contains_key(&k) {
        k = format!("{}#{}", key, n);
        n += 1;
    }
    now.0.insert(k, value);
}

/// What the walk of a crate found.
#[derive(Debug, Default)]
pub struct Measured {
    /// items over a limit, with their size
    pub sizes: Ratchet,
    /// statics with interior mutability: `static <file> <name>`, each 1
    pub statics: Ratchet,
    /// `unsafe <file>`: the count of `unsafe` in every file that has any
    pub unsafes: Ratchet,
}

/// Walks the crate in `root/krate`. Files under `src` only: integration tests are test code.
pub fn measure(root: &Path, krate: &str) -> Result<Measured, String> {
    let mut now = Ratchet::default();
    let mut statics = Ratchet::default();
    let (mut all_statics, mut types) = (Vec::new(), Vec::new());
    let mut unsafes = Ratchet::default();
    let mut spread: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in rust_files(&root.join(krate).join("src")) {
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", rel, e))?;
        let found = visit::walk(&text).map_err(|e| format!("{}: {}", rel, e))?;
        let lines = (text.lines().count() as u64).saturating_sub(found.test_lines);
        over(&mut now, format!("file {}", rel), lines, FILE_LINES);
        for (name, n) in found.fns {
            over(&mut now, format!("fn {} {}", rel, name), n, FN_LINES);
        }
        for (name, n) in found.structs {
            over(&mut now, format!("struct {} {}", rel, name), n, STRUCT_FIELDS);
        }
        for ty in found.impls {
            spread.entry(ty).or_default().insert(rel.clone());
        }
        over(&mut unsafes, format!("unsafe {}", rel), found.unsafes, 0);
        all_statics.extend(found.statics.into_iter().map(|(n, ty)| (rel.clone(), n, ty)));
        types.extend(found.types);
    }
    let interior = statics::interior_types(&types);
    for (rel, name, ty) in all_statics {
        if statics::mutable(&ty, &interior) {
            over(&mut statics, format!("static {} {}", rel, name), 1, 0);
        }
    }
    for (ty, files) in spread {
        over(&mut now, format!("impls {} {}", krate, ty), files.len() as u64, IMPL_FILES);
    }
    Ok(Measured { sizes: now, statics, unsafes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_items_over_the_limit_count_and_twins_get_a_suffix() {
        let mut r = Ratchet::default();
        over(&mut r, "fn a.rs f".into(), 60, FN_LINES);
        over(&mut r, "fn a.rs g".into(), 61, FN_LINES);
        over(&mut r, "fn a.rs g".into(), 70, FN_LINES);
        assert_eq!(r.0.keys().collect::<Vec<_>>(), ["fn a.rs g", "fn a.rs g#2"]);
    }

    #[test]
    fn xtask_keeps_its_own_limits() {
        let m = measure(&crate::repo::root(), "xtask").unwrap();
        assert!(m.sizes.0.is_empty() && m.statics.0.is_empty() && m.unsafes.0.is_empty(), "{:?}", m);
    }
}
