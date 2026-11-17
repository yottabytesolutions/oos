//! Size limits: files, functions, structs and the spread of a type's impl blocks. Only test code is exempt.

mod codelines;
pub mod marks;
mod statics;
pub mod unsafety;
mod visit;

use crate::ratchet::Ratchet;
use crate::sources::Source;
use std::collections::{BTreeMap, BTreeSet};

/// Lines of a source file outside `#[cfg(test)]` items, comment lines left out (ADR 0035).
pub const FILE_LINES: u64 = 400;
/// Code lines of a function, from `fn` to its closing brace: no blank and no comment lines (ADR 0010).
pub const FN_LINES: u64 = 60;
/// Fields of a struct.
pub const STRUCT_FIELDS: u64 = 12;
/// Files with inherent impl blocks of one type in a crate.
pub const IMPL_FILES: u64 = 3;

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
    /// `allow <file> clippy::disallowed_methods`: exceptions to the disallowed methods
    pub allows: Ratchet,
    /// `known <file> <test> <task>`: tests ignored as known failures
    pub known: Ratchet,
    /// `#[ignore = "known: .."]` without a task
    pub malformed: Vec<String>,
}

/// The walk of one source text, for tests of the checks built on it.
#[cfg(test)]
pub fn visit_for_test(text: &str) -> visit::Found {
    visit::walk(text).expect("parses")
}

/// Walks the sources of crate `krate` (`sources::of_dir`).
pub fn measure(krate: &str, sources: &[Source]) -> Measured {
    let mut now = Ratchet::default();
    let mut statics = Ratchet::default();
    let (mut all_statics, mut types) = (Vec::new(), Vec::new());
    let mut unsafes = Ratchet::default();
    let (mut allows, mut known, mut malformed) = (Ratchet::default(), Ratchet::default(), Vec::new());
    let mut spread: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for src in sources {
        let rel = &src.rel;
        let total = src.text.lines().count() as u64;
        let found = visit::walk_file(&src.file, &src.text, src.test);
        let lines = total.saturating_sub(found.test_lines + found.comment_lines);
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
        let m = marks::scan(&src.file);
        over(&mut allows, format!("allow {} {}", rel, marks::DISALLOWED), m.allows, 0);
        for (f, task) in m.known {
            over(&mut known, format!("known {} {} {:04}", rel, f, task), 1, 0);
        }
        malformed.extend(m.malformed.into_iter().map(|x| format!("{}: {}", rel, x)));
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
    Measured { sizes: now, statics, unsafes, allows, known, malformed }
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
        let root = crate::repo::root();
        let meta = crate::workspace::load(&Default::default(), &root).unwrap();
        let m = measure("xtask", &crate::sources::of_dir(&meta, "xtask").unwrap());
        assert!(m.sizes.0.is_empty() && m.statics.0.is_empty() && m.unsafes.0.is_empty(), "{:?}", m);
    }
}
