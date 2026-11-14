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
    /// every static outside tests: (`<enclosing fn>::NAME`, or `NAME` at module level; its type as token text)
    pub statics: Vec<(String, String)>,
    /// every struct and enum: (name, the token text of all its field types)
    pub types: Vec<(String, String)>,
    /// `unsafe` blocks, functions, impls and traits, test code included
    pub unsafes: u64,
}

/// The token text of all field types.
fn field_types<'a>(fields: impl Iterator<Item = &'a syn::Field>) -> String {
    fields.map(|f| quote_type(&f.ty)).collect::<Vec<_>>().join(" ")
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

/// The walk: `owner` is the impl or trait the next functions belong to.
#[derive(Debug, Default)]
pub struct Walk {
    pub found: Found,
    owner: Vec<String>,
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
        let name = match self.fns.last() {
            Some(f) => format!("{}::{}", f, s.ident),
            None => s.ident.to_string(),
        };
        self.found.statics.push((name, quote_type(&s.ty)));
        visit::visit_item_static(self, s);
    }

    fn visit_item_enum(&mut self, e: &'a syn::ItemEnum) {
        let fields = e.variants.iter().map(|v| field_types(v.fields.iter())).collect::<Vec<_>>().join(" ");
        self.found.types.push((e.ident.to_string(), fields));
        visit::visit_item_enum(self, e);
    }

    fn visit_item_struct(&mut self, s: &'a ItemStruct) {
        if is_test(&s.attrs) {
            return;
        }
        let n = match &s.fields {
            Fields::Named(f) => f.named.len(),
            Fields::Unnamed(f) => f.unnamed.len(),
            Fields::Unit => 0,
        };
        self.found.structs.push((s.ident.to_string(), n as u64));
        self.found.types.push((s.ident.to_string(), field_types(s.fields.iter())));
        visit::visit_item_struct(self, s);
    }
}

/// Walks the source `text`.
pub fn walk(text: &str) -> Result<Found, String> {
    let file = syn::parse_file(text).map_err(|e| format!("line {}: {}", e.span().start().line, e))?;
    let mut w = Walk::default();
    w.visit_file(&file);
    w.found.unsafes = super::unsafety::count(&file);
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
    fn statics_name_their_function_and_types_list_their_fields() {
        let src = "static A: AtomicU64 = AtomicU64::new(0);\n\
                   fn f() { static ON: OnceLock<bool> = OnceLock::new(); }\n\
                   impl S { fn g(&self) { static M: Mutex<u8> = Mutex::new(0); } }\n\
                   struct C { n: AtomicU64, k: u8 }\nenum E { A(Cell<u8>), B { x: u32 } }\n";
        let f = walk(src).unwrap();
        let names: Vec<&str> = f.statics.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["A", "f::ON", "S::g::M"]);
        assert_eq!(f.statics[1].1, "OnceLock < bool >");
        assert_eq!(
            f.types,
            [("C".to_string(), "AtomicU64 u8".to_string()), ("E".to_string(), "Cell < u8 > u32".to_string())]
        );
    }

    #[test]
    fn a_test_function_outside_a_test_module_is_skipped() {
        let f = walk("fn a() {}\n#[test]\nfn b() {\n}\n").unwrap();
        assert_eq!(f.fns.len(), 1);
        assert_eq!(f.test_lines, 2);
    }
}
