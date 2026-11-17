// SPDX-License-Identifier: GPL-2.0-or-later
//! The syntax walk of one file: functions, structs and inherent impls outside test code, with their sizes.

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use syn::visit::{self, Visit};
use syn::{Attribute, Fields, ImplItemFn, ItemFn, ItemImpl, ItemStatic, ItemStruct, TraitItemFn, Type};

/// The outer attributes at the front of an item's tokens.
struct Outer(Vec<Attribute>);

impl syn::parse::Parse for Outer {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        input.parse::<TokenStream>()?;
        Ok(Outer(attrs))
    }
}

/// The first and last line of `tokens`.
fn extent(tokens: TokenStream) -> Option<(usize, usize)> {
    let v: Vec<TokenTree> = tokens.into_iter().collect();
    let start = match v.first()? {
        TokenTree::Group(g) => g.span_open(),
        t => t.span(),
    };
    let end = match v.last()? {
        TokenTree::Group(g) => g.span_close(),
        t => t.span(),
    };
    Some((start.start().line, end.end().line))
}

/// What one file contains, test code left out.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// (qualified name, lines from `fn` to the closing brace)
    pub fns: Vec<(String, u64)>,
    /// (name, fields)
    pub structs: Vec<(String, u64)>,
    /// self types of inherent impl blocks
    pub impls: Vec<String>,
    /// lines inside `#[cfg(test)]` items and `#[test]` functions, their attributes included
    pub test_lines: u64,
    /// production code lines: lines with a token on them outside test code (`codelines`)
    pub code_lines: u64,
    /// comment lines outside test code: not blank and without a token (doc comments included)
    pub comment_lines: u64,
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

/// `#[test]`, or a `cfg` that requires test mode (`crate::sources::cfg_test`).
fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("test")) || crate::sources::cfg_test(attrs)
}

/// The tokens of a type as text.
fn quote_type(t: &Type) -> String {
    use quote::ToTokens;
    t.to_token_stream().to_string()
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
    /// the lines of the file with code on them (`codelines`)
    code: std::collections::BTreeSet<usize>,
    /// the qualified names of the enclosing functions
    fns: Vec<String>,
    /// the lines of test items
    test: std::collections::BTreeSet<usize>,
}

impl Walk {
    fn fn_name(&self, name: &syn::Ident) -> String {
        match self.owner.last() {
            Some(o) => format!("{}::{}", o, name),
            None => name.to_string(),
        }
    }

    /// Code lines from `start` to `end` (ADR 0010: no blank and no comment lines).
    fn code_lines(&self, start: proc_macro2::Span, end: proc_macro2::Span) -> u64 {
        self.code.range(start.start().line..=end.end().line).count() as u64
    }

    /// An item of any kind under `#[cfg(test)]` or `#[test]`: its lines, attributes included, are test lines.
    fn test_item(&mut self, item: &impl ToTokens) -> bool {
        let tokens = item.to_token_stream();
        let Ok(Outer(attrs)) = syn::parse2(tokens.clone()) else {
            return false;
        };
        match extent(tokens).filter(|_| is_test(&attrs)) {
            Some((start, end)) => {
                self.test.extend(start..=end);
                true
            }
            None => false,
        }
    }
}

impl<'a> Visit<'a> for Walk {
    fn visit_item(&mut self, i: &'a syn::Item) {
        if !self.test_item(i) {
            visit::visit_item(self, i);
        }
    }

    fn visit_impl_item(&mut self, i: &'a syn::ImplItem) {
        if !self.test_item(i) {
            visit::visit_impl_item(self, i);
        }
    }

    fn visit_trait_item(&mut self, i: &'a syn::TraitItem) {
        if !self.test_item(i) {
            visit::visit_trait_item(self, i);
        }
    }

    fn visit_stmt(&mut self, i: &'a syn::Stmt) {
        if !self.test_item(i) {
            visit::visit_stmt(self, i);
        }
    }

    fn visit_field(&mut self, i: &'a syn::Field) {
        if !self.test_item(i) {
            visit::visit_field(self, i);
        }
    }

    fn visit_variant(&mut self, i: &'a syn::Variant) {
        if !self.test_item(i) {
            visit::visit_variant(self, i);
        }
    }

    fn visit_arm(&mut self, i: &'a syn::Arm) {
        if !self.test_item(i) {
            visit::visit_arm(self, i);
        }
    }

    fn visit_field_value(&mut self, i: &'a syn::FieldValue) {
        if !self.test_item(i) {
            visit::visit_field_value(self, i);
        }
    }

    fn visit_item_fn(&mut self, f: &'a ItemFn) {
        let n = self.code_lines(f.sig.fn_token.span, f.block.brace_token.span.close());
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
        let ty = type_name(&i.self_ty);
        let owner = match &i.trait_ {
            Some((path, _)) => {
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
        let n = self.code_lines(f.sig.fn_token.span, f.block.brace_token.span.close());
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
            let n = self.code_lines(f.sig.fn_token.span, b.brace_token.span.close());
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
        let name = match self.fns.last() {
            Some(f) => format!("{}::{}", f, s.ident),
            None => s.ident.to_string(),
        };
        self.found.statics.push((name, quote_type(&s.ty)));
        visit::visit_item_static(self, s);
    }

    /// `type X = Mutex<u8>;`: an alias is a type whose one field is its definition.
    fn visit_item_type(&mut self, t: &'a syn::ItemType) {
        self.found.types.push((t.ident.to_string(), quote_type(&t.ty)));
        visit::visit_item_type(self, t);
    }

    /// `use std::sync::Mutex as M;`: `M` stands for `Mutex`.
    fn visit_use_rename(&mut self, r: &'a syn::UseRename) {
        self.found.types.push((r.rename.to_string(), r.ident.to_string()));
        visit::visit_use_rename(self, r);
    }

    fn visit_item_enum(&mut self, e: &'a syn::ItemEnum) {
        let fields = e.variants.iter().map(|v| field_types(v.fields.iter())).collect::<Vec<_>>().join(" ");
        self.found.types.push((e.ident.to_string(), fields));
        visit::visit_item_enum(self, e);
    }

    fn visit_item_struct(&mut self, s: &'a ItemStruct) {
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
#[cfg(test)]
pub fn walk(text: &str) -> Result<Found, String> {
    let file = syn::parse_file(text).map_err(|e| format!("line {}: {}", e.span().start().line, e))?;
    Ok(walk_file(&file, text, false))
}

/// Walks the parsed `file` of source `text`; a file of test code (`test`) only has its `unsafe` counted.
pub fn walk_file(file: &syn::File, text: &str, test: bool) -> Found {
    let unsafes = super::unsafety::count(file);
    if test {
        return Found { test_lines: text.lines().count() as u64, unsafes, ..Found::default() };
    }
    let mut w = Walk { code: super::codelines::of(file), ..Walk::default() };
    w.visit_file(file);
    let mut f = w.found;
    f.unsafes = unsafes;
    f.test_lines = w.test.len() as u64;
    for (i, line) in text.lines().enumerate().filter(|(i, _)| !w.test.contains(&(i + 1))) {
        if w.code.contains(&(i + 1)) {
            f.code_lines += 1;
        } else if !line.trim().is_empty() {
            f.comment_lines += 1;
        }
    }
    f
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
        assert_eq!(f.test_lines, 6);
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
        assert_eq!(f.test_lines, 3);
    }

    // The hole: a test-only const, struct or impl item was counted as production code.
    #[test]
    fn every_kind_of_test_item_is_test_code() {
        let src = "#[cfg(test)]\nconst K: [u8; 2] = [\n    1, 2,\n];\n#[cfg(test)]\nstruct S;\nimpl T {\n    #[cfg(test)]\n    const C: u8 = 1;\n    fn f() {}\n}\n";
        let f = walk(src).unwrap();
        assert_eq!((f.test_lines, f.code_lines), (8, 3));
        assert!(f.structs.is_empty());
    }

    // The holes: test-only fields, variants, statements, arms and compound cfgs counted as production code.
    #[test]
    fn test_gated_parts_and_compound_cfgs_are_test_code() {
        let src = "struct S {\n    a: u8,\n    #[cfg(test)]\n    b: u8,\n}\nenum E {\n    #[cfg(test)]\n    T,\n    U,\n}\nfn f(x: u8) -> S {\n    #[cfg(test)]\n    g();\n    match x {\n        #[cfg(test)]\n        1 => {}\n        _ => {}\n    }\n    S {\n        a: 1,\n        #[cfg(test)]\n        b: 2,\n    }\n}\n#[cfg(all(test, unix))]\nmod m {}\n";
        let f = walk(src).unwrap();
        assert_eq!(f.test_lines, 12);
    }

    #[test]
    fn code_and_comment_lines_leave_out_tests_and_blank_lines() {
        let src = "//! module doc\n/// item doc\nfn a() {\n\n    // note\n    let s = \"x\n\n y\";\n}\n/* block\n   end */\n#[cfg(test)]\n// test note\nmod tests {\n    fn t() {}\n}\n";
        let f = walk(src).unwrap();
        assert_eq!((f.code_lines, f.comment_lines, f.test_lines), (5, 5, 5));
    }
}
