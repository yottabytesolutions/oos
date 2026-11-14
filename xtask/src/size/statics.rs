//! Which statics hold interior mutability: a lock, an atomic or a cell in the written type, or in the fields of a
//! struct or enum of the crate that the type names (followed through further named types of the crate).
//! `OnceLock`, `LazyLock` and `OnceCell` around plain data are set once and then read only: not mutable state.

use std::collections::BTreeSet;

/// The identifiers in a type's token text.
fn words(ty: &str) -> impl Iterator<Item = &str> {
    ty.split(|c: char| !c.is_alphanumeric() && c != '_').filter(|w| !w.is_empty())
}

/// A lock, atomic or cell named directly in `ty`.
fn direct(ty: &str) -> bool {
    words(ty).any(|w| matches!(w, "Mutex" | "RwLock" | "Cell" | "RefCell" | "UnsafeCell") || w.starts_with("Atomic"))
}

/// The crate's types with interior mutability, from (name, field types) of every struct and enum: those with a lock,
/// atomic or cell in a field, then those with a field of such a type, until nothing changes.
pub fn interior_types(types: &[(String, String)]) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = types.iter().filter(|(_, f)| direct(f)).map(|(n, _)| n.clone()).collect();
    loop {
        let more: Vec<String> = types
            .iter()
            .filter(|(n, f)| !set.contains(n) && words(f).any(|w| set.contains(w)))
            .map(|(n, _)| n.clone())
            .collect();
        if more.is_empty() {
            return set;
        }
        set.extend(more);
    }
}

/// Whether a static of type `ty` holds interior mutability, given the crate's `interior` types.
pub fn mutable(ty: &str, interior: &BTreeSet<String>) -> bool {
    direct(ty) || words(ty).any(|w| interior.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(n: &str, f: &str) -> (String, String) {
        (n.to_string(), f.to_string())
    }

    #[test]
    fn named_types_are_followed_through_the_crate() {
        let types = [t("Counters", "AtomicU64 AtomicU64"), t("Outer", "Vec < Counters >"), t("Plain", "u8 String")];
        let i = interior_types(&types);
        assert_eq!(i.iter().collect::<Vec<_>>(), ["Counters", "Outer"]);
        assert!(mutable("OnceLock < Counters >", &i) && mutable("OnceLock < Outer >", &i));
        assert!(mutable("Mutex < u8 >", &i) && mutable("[AtomicU64 ; 3]", &i));
        assert!(!mutable("OnceLock < Plain >", &i) && !mutable("LazyLock < Vec < u8 > >", &i));
        assert!(!mutable("OnceCell < u8 >", &i) && mutable("RefCell < Vec < f32 > >", &i));
    }
}
