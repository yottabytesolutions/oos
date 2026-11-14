//! `unsafe` in a file: blocks, functions, impls, traits, extern blocks and unsafe attributes, test code included.

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

    /// `#[unsafe(no_mangle)]` and the attributes that are unsafe without saying so in edition 2021.
    fn visit_attribute(&mut self, a: &'a syn::Attribute) {
        let p = a.path();
        self.0 += ["unsafe", "no_mangle", "export_name", "link_section"].iter().any(|n| p.is_ident(n)) as u64;
        visit::visit_attribute(self, a);
    }

    fn visit_item_foreign_mod(&mut self, m: &'a syn::ItemForeignMod) {
        self.0 += m.unsafety.is_some() as u64;
        visit::visit_item_foreign_mod(self, m);
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
        // the hole: an unsafe extern block counted nothing
        assert_eq!(count(&syn::parse_file("unsafe extern \"C\" { fn e(); }\n").expect("parses")), 1);
        // the hole: unsafe attributes counted nothing
        let attrs = "#[unsafe(no_mangle)]\nfn a() {}\n#[unsafe(export_name = \"b\")]\nfn b() {}\n\
                     #[no_mangle]\nfn c() {}\n#[link_section = \".x\"]\nstatic S: u8 = 0;\n#[inline]\nfn d() {}\n";
        assert_eq!(count(&syn::parse_file(attrs).expect("parses")), 4);
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
