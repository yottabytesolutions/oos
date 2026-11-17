// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use syn::parse::Parser;

pub(super) fn comparison(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::Lt(_)
            | syn::BinOp::Le(_)
            | syn::BinOp::Gt(_)
            | syn::BinOp::Ge(_)
            | syn::BinOp::Eq(_)
            | syn::BinOp::Ne(_)
    )
}

pub(super) fn attrs(e: &syn::Expr) -> &[syn::Attribute] {
    match e {
        syn::Expr::Block(e) => &e.attrs,
        syn::Expr::Call(e) => &e.attrs,
        syn::Expr::MethodCall(e) => &e.attrs,
        syn::Expr::If(e) => &e.attrs,
        syn::Expr::Match(e) => &e.attrs,
        syn::Expr::Loop(e) => &e.attrs,
        syn::Expr::While(e) => &e.attrs,
        syn::Expr::ForLoop(e) => &e.attrs,
        syn::Expr::Macro(e) => &e.attrs,
        syn::Expr::Binary(e) => &e.attrs,
        syn::Expr::Assign(e) => &e.attrs,
        syn::Expr::Return(e) => &e.attrs,
        syn::Expr::Closure(e) => &e.attrs,
        syn::Expr::Path(e) => &e.attrs,
        syn::Expr::Paren(e) => &e.attrs,
        syn::Expr::Tuple(e) => &e.attrs,
        syn::Expr::Array(e) => &e.attrs,
        syn::Expr::Unary(e) => &e.attrs,
        syn::Expr::Reference(e) => &e.attrs,
        syn::Expr::Let(e) => &e.attrs,
        syn::Expr::Async(e) => &e.attrs,
        syn::Expr::Await(e) => &e.attrs,
        syn::Expr::Break(e) => &e.attrs,
        syn::Expr::Cast(e) => &e.attrs,
        syn::Expr::Const(e) => &e.attrs,
        syn::Expr::Continue(e) => &e.attrs,
        syn::Expr::Field(e) => &e.attrs,
        syn::Expr::Group(e) => &e.attrs,
        syn::Expr::Index(e) => &e.attrs,
        syn::Expr::Infer(e) => &e.attrs,
        syn::Expr::Lit(e) => &e.attrs,
        syn::Expr::Range(e) => &e.attrs,
        syn::Expr::RawAddr(e) => &e.attrs,
        syn::Expr::Repeat(e) => &e.attrs,
        syn::Expr::Struct(e) => &e.attrs,
        syn::Expr::Try(e) => &e.attrs,
        syn::Expr::TryBlock(e) => &e.attrs,
        syn::Expr::Unsafe(e) => &e.attrs,
        syn::Expr::Yield(e) => &e.attrs,
        _ => &[],
    }
}

#[derive(Default)]
struct ClockUse<'a> {
    names: Option<&'a BTreeSet<String>>,
    found: bool,
    now: bool,
}
impl<'a> Visit<'a> for ClockUse<'_> {
    fn visit_expr_call(&mut self, e: &'a syn::ExprCall) {
        if let syn::Expr::Path(p) = &*e.func {
            self.found |= self.now && p.path.segments.last().is_some_and(|s| s.ident == "now");
        }
        // Passing a clock value into a helper does not make its returned value a clock.
    }
    fn visit_expr_method_call(&mut self, e: &'a syn::ExprMethodCall) {
        self.found |= e.method == "elapsed";
        if e.method != "now" {
            self.visit_expr(&e.receiver);
        }
    }

    fn visit_expr_path(&mut self, e: &'a syn::ExprPath) {
        self.found |=
            self.names.is_some_and(|names| e.path.get_ident().is_some_and(|s| names.contains(&s.to_string())));
    }
}
pub(super) fn clock(e: &syn::Expr, names: &BTreeSet<String>) -> bool {
    let mut c = ClockUse { names: Some(names), now: true, ..Default::default() };
    c.visit_expr(e);
    c.found
}

pub(super) fn elapsed(e: &syn::Expr, names: &BTreeSet<String>) -> bool {
    let mut c = ClockUse { names: Some(names), ..Default::default() };
    c.visit_expr(e);
    c.found
}

pub(super) fn duration(e: &syn::Expr, values: &BTreeMap<String, u64>) -> Option<u64> {
    match e {
        syn::Expr::Path(p) => {
            let name = p.path.segments.last()?.ident.to_string();
            values.get(&name).copied().or_else(|| (name == "HANG_GUARD").then_some(120_000))
        }
        syn::Expr::Call(c) => {
            let syn::Expr::Path(p) = &*c.func else {
                return None;
            };
            let arg = c.args.first()?;
            let name = p.path.segments.last()?.ident.to_string();
            if name == "Some" {
                return duration(arg, values);
            }
            let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) = arg else {
                return None;
            };
            let n = n.base10_parse::<u64>().ok()?;
            match name.as_str() {
                "from_secs" => n.checked_mul(1000),
                "from_millis" => Some(n),
                "from_micros" => Some(n / 1000),
                "from_nanos" => Some(n / 1_000_000),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn macro_body(walk: &mut Walk, tokens: proc_macro2::TokenStream) {
    if let Ok(args) = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated.parse2(tokens.clone())
    {
        for e in &args {
            walk.visit_expr(e);
        }
    } else if let Ok(body) = syn::parse2::<syn::Block>(tokens.clone()) {
        walk.visit_block(&body);
    } else {
        // macro_rules patterns and arbitrary DSLs are not Rust expressions. Inspect token groups, ignoring literals.
        let mut previous = String::new();
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Ident(i) => previous = i.to_string(),
                proc_macro2::TokenTree::Group(g) => {
                    if previous == "sleep" || walk.sleeps.contains(&previous) {
                        walk.record("sleep");
                    }
                    macro_body(walk, g.stream());
                    previous.clear();
                }
                proc_macro2::TokenTree::Punct(p) if p.as_char() == ':' => {}
                _ => previous.clear(),
            }
        }
    }
}
