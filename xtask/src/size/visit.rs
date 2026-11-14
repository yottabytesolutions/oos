//! The syntax walk of one file: functions, structs and inherent impls outside test code, with their sizes.

use syn::visit::{self, Visit};
use syn::{Attribute, Fields, ImplItemFn, ItemFn, ItemImpl, ItemMod, ItemStatic, ItemStruct, TraitItemFn, Type};

/// What one file contains, test code left out.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// (qualified name, lines from `fn` to the closing brace)
    pub fns: Vec<(String, u64)>,
    /// (name, fields)
    pub structs: Vec<(String, u64)>,
    /// self types of inherent impl blocks
    pub impls: Vec<String>,
    /// lines inside `#[cfg(test)]` items and `#[test]` functions
    pub test_lines: u64,
    /// statics with interior mutability, as `<enclosing fn>::NAME` (`NAME` at module level)
    pub statics: Vec<String>,
}

/// Whether a type (as token text) has interior mutability: a lock, an atomic or a cell anywhere in it.
/// `OnceLock`, `LazyLock` and `OnceCell` around plain data are set once and then read only: not mutable state.
/// Only the written type counts: a struct of atomics behind its own name is not seen.
fn interior(ty: &str) -> bool {
    ty.split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|w| matches!(w, "Mutex" | "RwLock" | "Cell" | "RefCell" | "UnsafeCell") || w.starts_with("Atomic"))
}

/// `#[cfg(test)]` or `#[test]`.
fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || (a.path().is_ident("cfg") && a.meta.require_list().is_ok_and(|l| l.tokens.to_string().trim() == "test"))
    })
}

/// The tokens of a type as text.
fn quote_type(t: &Type) -> String {
    use quote::ToTokens;
    t.to_token_stream().to_string()
}

/// Lines from `start` to `end` inclusive.
fn lines(start: proc_macro2::Span, end: proc_macro2::Span) -> u64 {
    (end.end().line + 1).saturating_sub(start.start().line) as u64
}

/// The last path segment of a type: `Ui` for `crate::ui::Ui<T>`.
fn type_name(t: &Type) -> String {
    match t {
        Type::Path(p) => p.path.segments.last().map_or_else(|| "?".into(), |s| s.ident.to_string()),
        Type::Reference(r) => type_name(&r.elem),
        _ => "?".into(),
    }
}

/// The line above a plain data record: exempt from the field limit (docs/development.md).
pub const RECORD_MARK: &str = "// xtask: record";

/// The walk: `owner` is the impl or trait the next functions belong to; `marks` the lines (1-based) holding
/// `RECORD_MARK`.
#[derive(Debug, Default)]
pub struct Walk {
    pub found: Found,
    owner: Vec<String>,
    marks: Vec<usize>,
    /// the qualified names of the enclosing functions
    fns: Vec<String>,
}

impl Walk {
    fn fn_name(&self, name: &syn::Ident) -> String {
        match self.owner.last() {
            Some(o) => format!("{}::{}", o, name),
            None => name.to_string(),
        }
    }

    /// A record mark between the struct's first attribute (or doc comment) line minus one and its `struct` line.
    fn record(&self, s: &ItemStruct) -> bool {
        let at = s.struct_token.span.start().line;
        let first = s.attrs.first().map_or(at, |a| a.pound_token.span.start().line);
        self.marks.iter().any(|&m| m + 1 >= first && m <= at)
    }

    fn skip(&mut self, attrs: &[Attribute], start: proc_macro2::Span, end: proc_macro2::Span) -> bool {
        let t = is_test(attrs);
        if t {
            self.found.test_lines += lines(start, end);
        }
        t
    }
}

impl<'a> Visit<'a> for Walk {
    fn visit_item_mod(&mut self, m: &'a ItemMod) {
        let end = m.content.as_ref().map_or(m.semi.map_or(m.ident.span(), |s| s.span), |(b, _)| b.span.close());
        if !self.skip(&m.attrs, m.mod_token.span, end) {
            visit::visit_item_mod(self, m);
        }
    }

    fn visit_item_fn(&mut self, f: &'a ItemFn) {
        if self.skip(&f.attrs, f.sig.fn_token.span, f.block.brace_token.span.close()) {
            return;
        }
        let n = lines(f.sig.fn_token.span, f.block.brace_token.span.close());
        let name = self.fn_name(&f.sig.ident);
        self.found.fns.push((name.clone(), n));
        // a nested function counts on its own, outside any impl
        let outer = std::mem::take(&mut self.owner);
        self.fns.push(name);
        visit::visit_item_fn(self, f);
        self.fns.pop();
        self.owner = outer;
    }

    fn visit_item_impl(&mut self, i: &'a ItemImpl) {
        if self.skip(&i.attrs, i.impl_token.span, i.brace_token.span.close()) {
            return;
        }
        let ty = type_name(&i.self_ty);
        let owner = match &i.trait_ {
            Some((_, path, _)) => {
                let tr = path.segments.last().map_or_else(|| "?".into(), |s| s.ident.to_string());
                format!("<{} as {}>", ty, tr)
            }
            None => {
                self.found.impls.push(ty.clone());
                ty
            }
        };
        self.owner.push(owner);
        visit::visit_item_impl(self, i);
        self.owner.pop();
    }

    fn visit_impl_item_fn(&mut self, f: &'a ImplItemFn) {
        if self.skip(&f.attrs, f.sig.fn_token.span, f.block.brace_token.span.close()) {
            return;
        }
        let n = lines(f.sig.fn_token.span, f.block.brace_token.span.close());
        let name = self.fn_name(&f.sig.ident);
        self.found.fns.push((name.clone(), n));
        let outer = std::mem::take(&mut self.owner);
        self.fns.push(name);
        visit::visit_impl_item_fn(self, f);
        self.fns.pop();
        self.owner = outer;
    }

    fn visit_trait_item_fn(&mut self, f: &'a TraitItemFn) {
        if let Some(b) = &f.default {
            let n = lines(f.sig.fn_token.span, b.brace_token.span.close());
            self.found.fns.push((self.fn_name(&f.sig.ident), n));
        }
        visit::visit_trait_item_fn(self, f);
    }

    fn visit_item_trait(&mut self, t: &'a syn::ItemTrait) {
        self.owner.push(t.ident.to_string());
        visit::visit_item_trait(self, t);
        self.owner.pop();
    }

    fn visit_item_static(&mut self, s: &'a ItemStatic) {
        if is_test(&s.attrs) {
            return;
        }
        if interior(&quote_type(&s.ty)) {
            let name = match self.fns.last() {
                Some(f) => format!("{}::{}", f, s.ident),
                None => s.ident.to_string(),
            };
            self.found.statics.push(name);
        }
        visit::visit_item_static(self, s);
    }

    fn visit_item_struct(&mut self, s: &'a ItemStruct) {
        if is_test(&s.attrs) || self.record(s) {
            return;
        }
        let n = match &s.fields {
            Fields::Named(f) => f.named.len(),
            Fields::Unnamed(f) => f.unnamed.len(),
            Fields::Unit => 0,
        };
        self.found.structs.push((s.ident.to_string(), n as u64));
        visit::visit_item_struct(self, s);
    }
}

/// Walks the source `text`.
pub fn walk(text: &str) -> Result<Found, String> {
    let file = syn::parse_file(text).map_err(|e| format!("line {}: {}", e.span().start().line, e))?;
    let marks = text.lines().enumerate().filter(|(_, l)| l.trim() == RECORD_MARK).map(|(i, _)| i + 1).collect();
    let mut w = Walk { marks, ..Walk::default() };
    w.visit_file(&file);
    Ok(w.found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "struct S { a: u8, b: u8 }
impl S {
    fn one(&self) -> u8 {
        fn inner() {}
        inner();
        self.a
    }
}
impl Clone for S {
    fn clone(&self) -> S {
        S { a: self.a, b: self.b }
    }
}
trait T {
    fn d(&self) {}
}
fn free() {}
#[cfg(test)]
mod tests {
    fn helper() {}
    #[test]
    fn t() {}
}
";

    #[test]
    fn finds_functions_structs_and_impls_outside_tests() {
        let f = walk(SRC).unwrap();
        let names: Vec<&str> = f.fns.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["S::one", "inner", "<S as Clone>::clone", "T::d", "free"]);
        assert_eq!(f.fns[0].1, 5);
        assert_eq!(f.structs, vec![("S".to_string(), 2)]);
        assert_eq!(f.impls, vec!["S".to_string()]);
        assert_eq!(f.test_lines, 5);
    }

    #[test]
    fn statics_with_interior_mutability_name_their_function() {
        let src = "static A: AtomicU64 = AtomicU64::new(0);\nstatic B: u32 = 1;\n\
                   fn f() { static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new(); }\n\
                   fn h() { static S: OnceLock<Mutex<u8>> = OnceLock::new(); }\n\
                   impl S { fn g(&self) { static M: Mutex<u8> = Mutex::new(0); } }\n";
        assert_eq!(walk(src).unwrap().statics, ["A", "h::S", "S::g::M"]);
        assert!(
            interior("RefCell < Vec < f32 > >")
                && !interior("OnceCell < u8 >")
                && !interior("LazyLock < Vec < Proto > >")
        );
    }

    #[test]
    fn a_marked_record_has_no_field_limit() {
        let f = walk("// xtask: record\n/// doc\n#[derive(Debug)]\nstruct A { x: u8 }\nstruct B { y: u8 }\n").unwrap();
        assert_eq!(f.structs, vec![("B".to_string(), 1)]);
        let g = walk("/// doc\n// xtask: record\npub struct A(u8);\n// xtask: record\n\nstruct C;\n").unwrap();
        assert_eq!(g.structs, vec![("C".to_string(), 0)]);
    }

    #[test]
    fn a_test_function_outside_a_test_module_is_skipped() {
        let f = walk("fn a() {}\n#[test]\nfn b() {\n}\n").unwrap();
        assert_eq!(f.fns.len(), 1);
        assert_eq!(f.test_lines, 2);
    }
}
