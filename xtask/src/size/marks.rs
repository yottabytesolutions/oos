// SPDX-License-Identifier: GPL-2.0-or-later
//! Marks that may only become fewer: `#[allow]`/`#[expect]` of `clippy::disallowed_methods`, and known failures,
//! tests ignored with `#[ignore = "known: task NNNN"]`. Test code included. Every `#[ignore]` carries a reason, and a
//! `flaky:` reason names its issue (`#NNN`): a flaky test is tracked, never silently parked.

use crate::attrs;
use syn::visit::{self, Visit};

/// The lint whose exceptions are counted.
pub const DISALLOWED: &str = "clippy::disallowed_methods";

#[derive(Debug, Default)]
pub struct Marks {
    /// allow/expect attributes naming `DISALLOWED`
    pub allows: u64,
    /// (test function, task number) of every known failure
    pub known: Vec<(String, u32)>,
    /// ignores without a reason, `known:` reasons without a task, `flaky:` reasons without an issue
    pub malformed: Vec<String>,
}

/// The task of a known-failure reason: 12 for `known: task 0012`.
fn task_of(reason: &str) -> Option<u32> {
    reason.strip_prefix("known: task ")?.trim().parse().ok()
}

/// Whether `reason` names an issue: `#` followed by digits.
fn names_issue(reason: &str) -> bool {
    reason.split('#').skip(1).any(|r| r.starts_with(|c: char| c.is_ascii_digit()))
}

impl Marks {
    fn attrs(&mut self, a: &[syn::Attribute], fn_name: Option<&syn::Ident>) {
        for x in a.iter().flat_map(attrs::applied) {
            if matches!(x.name.as_str(), "allow" | "expect") {
                self.allows += attrs::list_paths(&x).iter().filter(|p| *p == DISALLOWED).count() as u64;
            }
            let (Some(f), "ignore") = (fn_name, x.name.as_str()) else { continue };
            let Some(v) = attrs::string_value(&x) else {
                self.malformed.push(format!("{}: #[ignore] needs a reason: #[ignore = \"...\"]", f));
                continue;
            };
            match (v.starts_with("known"), task_of(&v)) {
                (_, Some(t)) => self.known.push((f.to_string(), t)),
                (true, None) => self.malformed.push(format!("{}: {:?}: \"known: task NNNN\" expected", f, v)),
                _ if v.starts_with("flaky") && !names_issue(&v) => {
                    self.malformed.push(format!("{}: {:?}: \"flaky: #NNN ...\" expected", f, v))
                }
                _ => {}
            }
        }
    }
}

impl<'a> Visit<'a> for Marks {
    fn visit_item_fn(&mut self, f: &'a syn::ItemFn) {
        self.attrs(&f.attrs, Some(&f.sig.ident));
        visit::visit_block(self, &f.block);
    }

    fn visit_impl_item_fn(&mut self, f: &'a syn::ImplItemFn) {
        self.attrs(&f.attrs, Some(&f.sig.ident));
        visit::visit_block(self, &f.block);
    }

    fn visit_attribute(&mut self, a: &'a syn::Attribute) {
        self.attrs(std::slice::from_ref(a), None);
    }
}

/// The marks of a parsed file.
pub fn scan(file: &syn::File) -> Marks {
    let mut m = Marks::default();
    m.visit_file(file);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_disallowed_exceptions_and_known_failures() {
        let src = "#![allow(clippy::disallowed_methods)]\n\
                   #[cfg_attr(all(), expect(clippy::disallowed_methods))]\nfn a() {}\n\
                   #[allow(dead_code)]\nfn b() {}\n\
                   #[test]\n#[ignore = \"known: task 0012\"]\nfn c() {}\n\
                   #[test]\n#[ignore]\nfn d() {}\n\
                   #[test]\n#[ignore = \"known: flaky\"]\nfn e() {}\n";
        let m = scan(&syn::parse_file(src).expect("parses"));
        assert_eq!(m.allows, 2);
        assert_eq!(m.known, [("c".to_string(), 12)]);
        assert_eq!(m.malformed.len(), 2);
        assert!(m.malformed[0].starts_with("d: #[ignore] needs a reason"));
    }

    #[test]
    fn flaky_ignores_name_an_issue() {
        let src = "#[test]\n#[ignore = \"flaky: #123 races the timer\"]\nfn a() {}\n\
                   #[test]\n#[ignore = \"flaky: races the timer\"]\nfn b() {}\n\
                   #[test]\n#[ignore = \"flaky: see # later\"]\nfn c() {}\n\
                   #[test]\n#[ignore = \"needs a unit\"]\nfn d() {}\n";
        let m = scan(&syn::parse_file(src).expect("parses"));
        let bad: Vec<_> = m.malformed.iter().map(|x| x.split(':').next().unwrap_or("")).collect();
        assert_eq!(bad, ["b", "c"]);
    }
}
