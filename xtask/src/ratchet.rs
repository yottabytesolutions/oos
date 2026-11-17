// SPDX-License-Identifier: GPL-2.0-or-later
//! A ratchet: findings that existed when a rule came in, each with its size or count. The gate fails when a listed
//! entry grows or an entry appears that is not listed; an entry that shrank or went away may be written back
//! (`cargo xtask ratchet`), never raised.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Entries by key: the allowed value (size or count).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Ratchet(pub BTreeMap<String, u64>);

/// The comparison of the current state with a ratchet.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Verdict {
    /// listed and now larger: (key, allowed, now)
    pub grown: Vec<(String, u64, u64)>,
    /// not listed and over the limit: (key, now)
    pub new: Vec<(String, u64)>,
    /// listed and now smaller or gone: (key, allowed, now)
    pub shrunk: Vec<(String, u64, u64)>,
}

impl Verdict {
    /// One line per failure, for the report.
    pub fn failures(&self) -> Vec<String> {
        let grown = self.grown.iter().map(|(k, a, n)| format!("{} grew: {} -> {} (ratchet)", k, a, n));
        let new = self.new.iter().map(|(k, n)| format!("{} is {} (new: fix it, it cannot be listed)", k, n));
        grown.chain(new).collect()
    }
}

impl Ratchet {
    /// Parses `<value> <key>` lines; `#` starts a comment line.
    pub fn parse(text: &str) -> Result<Ratchet, String> {
        let mut m = BTreeMap::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (v, k) = line.split_once(' ').ok_or_else(|| format!("line {}: no '<value> <key>'", i + 1))?;
            let v = v.parse().map_err(|_| format!("line {}: {:?} is no count", i + 1, v))?;
            m.insert(k.trim().to_string(), v);
        }
        Ok(Ratchet(m))
    }

    /// Reads a ratchet file; a missing file is an empty ratchet.
    pub fn load(path: &Path) -> Result<Ratchet, String> {
        match std::fs::read_to_string(path) {
            Ok(t) => Ratchet::parse(&t).map_err(|e| format!("{}: {}", path.display(), e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ratchet::default()),
            Err(e) => Err(format!("{}: {}", path.display(), e)),
        }
    }

    /// The file text: `header` as comment lines, then the entries sorted by key.
    pub fn text(&self, header: &str) -> String {
        let mut s: String = header.lines().map(|l| format!("# {}\n", l)).collect();
        for (k, v) in &self.0 {
            let _ = writeln!(s, "{} {}", v, k);
        }
        s
    }

    /// Compares `now` (every entry over the limit, with its value) with the ratchet.
    pub fn compare(&self, now: &Ratchet) -> Verdict {
        let mut v = Verdict::default();
        for (k, &n) in &now.0 {
            match self.0.get(k) {
                Some(&a) if n > a => v.grown.push((k.clone(), a, n)),
                Some(&a) if n < a => v.shrunk.push((k.clone(), a, n)),
                Some(_) => {}
                None => v.new.push((k.clone(), n)),
            }
        }
        for (k, &a) in &self.0 {
            if !now.0.contains_key(k) {
                v.shrunk.push((k.clone(), a, 0));
            }
        }
        v
    }

    /// The ratchet after `now`: entries lowered to their current value, gone entries dropped. Never raises or adds.
    pub fn tightened(&self, now: &Ratchet) -> Ratchet {
        Ratchet(self.0.iter().filter_map(|(k, &a)| now.0.get(k).map(|&n| (k.clone(), n.min(a)))).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(e: &[(&str, u64)]) -> Ratchet {
        Ratchet(e.iter().map(|(k, v)| (k.to_string(), *v)).collect())
    }

    #[test]
    fn parses_and_writes_back() {
        let t = "# header\n\n12 fn ui/src/ui.rs Ui::step\n3 file a.rs\n";
        let x = Ratchet::parse(t).unwrap();
        assert_eq!(x, r(&[("fn ui/src/ui.rs Ui::step", 12), ("file a.rs", 3)]));
        assert_eq!(Ratchet::parse(&x.text("header")).unwrap(), x);
        assert!(Ratchet::parse("x key").is_err() && Ratchet::parse("12").is_err());
    }

    #[test]
    fn growth_and_new_entries_fail_shrinking_passes() {
        let allowed = r(&[("a", 10), ("b", 5), ("c", 7)]);
        let v = allowed.compare(&r(&[("a", 11), ("b", 4), ("d", 1)]));
        assert_eq!(v.grown, vec![("a".to_string(), 10, 11)]);
        assert_eq!(v.new, vec![("d".to_string(), 1)]);
        assert_eq!(v.shrunk, vec![("b".to_string(), 5, 4), ("c".to_string(), 7, 0)]);
        assert_eq!(v.failures().len(), 2);
        assert!(allowed.compare(&r(&[("a", 10), ("b", 1)])).failures().is_empty());
    }

    #[test]
    fn tightening_never_raises_or_adds() {
        let allowed = r(&[("a", 10), ("b", 5), ("c", 7)]);
        assert_eq!(allowed.tightened(&r(&[("a", 12), ("b", 4), ("d", 9)])), r(&[("a", 10), ("b", 4)]));
    }
}
