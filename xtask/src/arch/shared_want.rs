// SPDX-License-Identifier: GPL-2.0-or-later
//! Direct acquisition-settings writes belong to the change point, including mutable borrows.
//! This is a syntax fence, like the module rules, rather than Rust type inference.

use crate::sources::Source;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{Expr, Member};

pub(super) fn check(sources: &[Source]) -> Vec<String> {
    let mut out = Vec::new();
    for s in sources {
        if s.test || s.module == "ui::change::acquisition" {
            continue;
        }
        let mut writes = Writes { source: s, out: &mut out };
        writes.visit_file(&s.file);
    }
    out
}

struct Writes<'a> {
    source: &'a Source,
    out: &'a mut Vec<String>,
}

impl Writes<'_> {
    fn target(&mut self, expr: &Expr) {
        match expr {
            Expr::Field(f) if matches!(&f.member, Member::Named(n) if n.unraw() == "want") => {
                if !self.unrelated(&f.base) {
                    self.out.push(format!(
                        "{}:{}: only ui::change::acquisition may write Shared.want",
                        self.source.rel,
                        f.member.span().start().line
                    ));
                }
            }
            Expr::Field(f) => self.target(&f.base),
            Expr::Index(i) => self.target(&i.expr),
            Expr::Paren(p) => self.target(&p.expr),
            Expr::Group(g) => self.target(&g.expr),
            Expr::Unary(u) => self.target(&u.expr),
            Expr::Tuple(t) => t.elems.iter().for_each(|e| self.target(e)),
            _ => {}
        }
    }

    fn unrelated(&self, base: &Expr) -> bool {
        // Segmented browse requests, autoset's channel-selection array and the GPU preference
        // are separate fields. Keep exemptions tied to their exact receiver and owning module.
        matches!(base, Expr::Field(f) if matches!(&f.member, Member::Named(n) if n == "seg"))
            || matches!(base, Expr::Path(p) if
                (self.source.module == "ui::autoset" && p.path.is_ident("a")) ||
                (self.source.module == "system::device::graphics" && p.path.is_ident("self")))
    }
}

use syn::spanned::Spanned;

impl<'ast> Visit<'ast> for Writes<'_> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let test = match item {
            syn::Item::Mod(m) => &m.attrs,
            syn::Item::Fn(f) => &f.attrs,
            _ => return visit::visit_item(self, item),
        };
        if !test
            .iter()
            .any(|a| a.path().is_ident("cfg") && a.meta.require_list().is_ok_and(|l| l.tokens.to_string() == "test"))
        {
            visit::visit_item(self, item);
        }
    }

    fn visit_expr_assign(&mut self, e: &'ast syn::ExprAssign) {
        self.target(&e.left);
        visit::visit_expr_assign(self, e);
    }

    fn visit_expr_binary(&mut self, e: &'ast syn::ExprBinary) {
        if matches!(
            e.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
                | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_)
                | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_)
                | syn::BinOp::ShrAssign(_)
        ) {
            self.target(&e.left);
        }
        visit::visit_expr_binary(self, e);
    }

    fn visit_expr_reference(&mut self, e: &'ast syn::ExprReference) {
        if e.mutability.is_some() {
            self.target(&e.expr);
        }
        visit::visit_expr_reference(self, e);
    }

    fn visit_expr_method_call(&mut self, e: &'ast syn::ExprMethodCall) {
        // Option's read-only methods used by acquisition consumers. Unknown methods fail closed.
        if !matches!(e.method.to_string().as_str(), "clone" | "as_ref" | "is_some" | "is_none") {
            self.target(&e.receiver);
        }
        visit::visit_expr_method_call(self, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(module: &str, text: &str) -> Source {
        Source {
            rel: "ui/src/fixture.rs".into(),
            module: module.into(),
            test: false,
            text: text.into(),
            file: syn::parse_file(text).unwrap(),
        }
    }

    #[test]
    fn rejects_writes_borrows_and_mutating_methods_but_keeps_reads_and_tests() {
        for body in [
            "g.want = Some(w);",
            "g.r#want = Some(w);",
            "g.want += w;",
            "(g.want, g.slow) = (Some(w), s);",
            "g.want.take();",
            "g.want.replace(w);",
            "g.want.as_mut();",
            "consume(&mut g.want);",
            "g.want.trigger = t;",
        ] {
            let s = source("ui::bus_candidate", &format!("fn f() {{ {body} }}"));
            assert!(!check(&[s]).is_empty(), "{body}");
        }
        for (module, body) in [
            ("ui::change::acquisition", "fn f() { g.want = Some(w); }"),
            ("board::dho", "fn f() { g.want.clone(); g.want.as_ref(); }"),
            ("ui::bus_candidate", "#[cfg(test)] mod tests { fn f() { g.want = Some(w); } }"),
            ("board::dho::hw::segmented", "fn f() { g.seg.want = Some(k); }"),
            ("ui::autoset", "fn f() { a.want[c] = true; }"),
            ("system::device::graphics", "fn f() { self.want = true; }"),
        ] {
            assert!(check(&[source(module, body)]).is_empty(), "{body}");
        }
    }
}
