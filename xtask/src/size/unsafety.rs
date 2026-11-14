//! `unsafe` in a file: blocks, functions, impls and traits, test code included.

use syn::visit::{self, Visit};

#[derive(Debug, Default)]
struct Count(u64);

impl<'a> Visit<'a> for Count {
    fn visit_expr_unsafe(&mut self, e: &'a syn::ExprUnsafe) {
        self.0 += 1;
        visit::visit_expr_unsafe(self, e);
    }

    fn visit_signature(&mut self, s: &'a syn::Signature) {
        self.0 += s.unsafety.is_some() as u64;
        visit::visit_signature(self, s);
    }

    fn visit_item_impl(&mut self, i: &'a syn::ItemImpl) {
        self.0 += i.unsafety.is_some() as u64;
        visit::visit_item_impl(self, i);
    }

    fn visit_item_trait(&mut self, t: &'a syn::ItemTrait) {
        self.0 += t.unsafety.is_some() as u64;
        visit::visit_item_trait(self, t);
    }
}

/// The `unsafe` blocks, functions, impls and traits in `file`.
pub fn count(file: &syn::File) -> u64 {
    let mut c = Count::default();
    c.visit_file(file);
    c.0
}

/// Whether `path` (relative to the root) is under `prefix`: the same file, a directory below it, or the module
/// file `<prefix>.rs`.
pub fn under(path: &str, prefix: &str) -> bool {
    let p = prefix.trim_end_matches('/');
    path == p || path.starts_with(&format!("{}/", p)) || path == format!("{}.rs", p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_every_kind_of_unsafe() {
        let f = syn::parse_file(
            "unsafe fn a() {}\nfn b() { unsafe { a() } }\nunsafe impl Send for X {}\nunsafe trait T {}\n\
             extern \"C\" { fn c(); }\n#[cfg(test)]\nmod t { fn d() { unsafe { } } }\n",
        )
        .expect("parses");
        assert_eq!(count(&f), 5);
    }

    #[test]
    fn prefixes_cover_files_dirs_and_module_files() {
        assert!(under("hal/src/dho/bus.rs", "hal") && under("ui/src/input.rs", "ui/src/input"));
        assert!(
            under("ui/src/board/dho/fpga.rs", "ui/src/board/dho/") && under("ui/src/fastdraw.rs", "ui/src/fastdraw.rs")
        );
        assert!(!under("ui/src/inputs.rs", "ui/src/input") && !under("halt/x.rs", "hal"));
    }
}
