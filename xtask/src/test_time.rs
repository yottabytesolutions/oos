// SPDX-License-Identifier: GPL-2.0-or-later
//! Test timing source guard, including cfg(test) expressions and macro bodies.
use crate::{ratchet::Ratchet, sources, steps, store::Kind, workspace};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use syn::visit::{self, Visit};
mod expressions;

#[derive(Default)]
struct Walk {
    test: bool,
    clock_fork: bool,
    names: Vec<String>,
    found: Ratchet,
    sleeps: BTreeSet<String>,
    clocks: BTreeSet<String>,
    elapsed: BTreeSet<String>,
    durations: BTreeMap<String, u64>,
}
fn test(attrs: &[syn::Attribute]) -> bool {
    crate::attrs::any(attrs, &["test"]) || crate::sources::cfg_test(attrs)
}
fn sleep_aliases(tree: &syn::UseTree, out: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Rename(r) if r.ident == "sleep" => {
            out.insert(r.rename.to_string());
        }
        syn::UseTree::Path(p) => sleep_aliases(&p.tree, out),
        syn::UseTree::Group(g) => g.items.iter().for_each(|t| sleep_aliases(t, out)),
        _ => {}
    }
}
impl Walk {
    fn record(&mut self, kind: &str) {
        if self.test {
            *self.found.0.entry(format!("{} {}", self.names.join("::"), kind)).or_default() += 1;
        }
    }
    fn timeout<'a>(&mut self, name: &str, mut args: impl DoubleEndedIterator<Item = &'a syn::Expr>) {
        if name.ends_with("_timeout")
            && args.next_back().and_then(|e| expressions::duration(e, &self.durations)).is_none_or(|n| n < 120_000)
        {
            self.record("short-timeout");
        }
    }
    fn function(&mut self, attrs: &[syn::Attribute], name: String, body: &syn::Block) {
        let saved = self.test;
        let clocks = self.clocks.clone();
        let elapsed = self.elapsed.clone();
        let durations = self.durations.clone();
        self.test |= test(attrs);
        self.names.push(name);
        self.visit_block(body);
        self.names.pop();
        self.test = saved;
        self.clocks = clocks;
        self.elapsed = elapsed;
        self.durations = durations;
    }
}
impl<'a> Visit<'a> for Walk {
    fn visit_item_use(&mut self, item: &'a syn::ItemUse) {
        sleep_aliases(&item.tree, &mut self.sleeps);
    }
    fn visit_item_mod(&mut self, item: &'a syn::ItemMod) {
        let saved = self.test;
        self.test |= test(&item.attrs);
        self.names.push(item.ident.to_string());
        visit::visit_item_mod(self, item);
        self.names.pop();
        self.test = saved;
    }
    fn visit_item_fn(&mut self, item: &'a syn::ItemFn) {
        self.function(&item.attrs, item.sig.ident.to_string(), &item.block);
    }
    fn visit_item_impl(&mut self, item: &'a syn::ItemImpl) {
        let saved = self.test;
        self.test |= test(&item.attrs);
        visit::visit_item_impl(self, item);
        self.test = saved;
    }
    fn visit_impl_item_fn(&mut self, item: &'a syn::ImplItemFn) {
        self.function(&item.attrs, item.sig.ident.to_string(), &item.block);
    }
    fn visit_local(&mut self, local: &'a syn::Local) {
        let saved = self.test;
        self.test |= test(&local.attrs);
        let clock_fork = self.clock_fork;
        self.clock_fork |= !saved && self.test;
        if let (syn::Pat::Ident(p), Some(init)) = (&local.pat, &local.init) {
            if expressions::elapsed(&init.expr, &self.elapsed) {
                self.elapsed.insert(p.ident.to_string());
            }
            if expressions::clock(&init.expr, &self.clocks) {
                self.clocks.insert(p.ident.to_string());
            }
            if let Some(n) = expressions::duration(&init.expr, &self.durations) {
                self.durations.insert(p.ident.to_string(), n);
            }
        }
        visit::visit_local(self, local);
        self.test = saved;
        self.clock_fork = clock_fork;
    }
    fn visit_item_const(&mut self, item: &'a syn::ItemConst) {
        if let Some(n) = expressions::duration(&item.expr, &self.durations) {
            self.durations.insert(item.ident.to_string(), n);
        }
        visit::visit_item_const(self, item);
    }
    fn visit_expr(&mut self, expr: &'a syn::Expr) {
        let saved = self.test;
        self.test |= test(expressions::attrs(expr));
        let clock_fork = self.clock_fork;
        self.clock_fork |= !saved && self.test;
        visit::visit_expr(self, expr);
        self.test = saved;
        self.clock_fork = clock_fork;
    }
    fn visit_expr_binary(&mut self, e: &'a syn::ExprBinary) {
        if expressions::comparison(&e.op) && expressions::clock(&syn::Expr::Binary(e.clone()), &self.clocks) {
            self.record("clock-comparison");
        }
        visit::visit_expr_binary(self, e);
    }
    fn visit_expr_call(&mut self, e: &'a syn::ExprCall) {
        if let syn::Expr::Path(p) = &*e.func {
            if let Some(s) = p.path.segments.last() {
                let name = s.ident.to_string();
                if self.clock_fork && name == "now" {
                    self.record("test-clock-fork");
                }
                if name == "sleep" || self.sleeps.contains(&name) {
                    self.record("sleep");
                }
                self.timeout(&name, e.args.iter());
            }
        }
        visit::visit_expr_call(self, e);
    }
    fn visit_expr_method_call(&mut self, e: &'a syn::ExprMethodCall) {
        if self.clock_fork && (e.method == "elapsed" || e.method == "now") {
            self.record("test-clock-fork");
        }
        self.timeout(&e.method.to_string(), e.args.iter());
        visit::visit_expr_method_call(self, e);
    }
    fn visit_macro(&mut self, m: &'a syn::Macro) {
        use syn::parse::Parser;
        if m.path.is_ident("assert") {
            if let Ok(args) =
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated.parse2(m.tokens.clone())
            {
                if args.first().is_some_and(|e| {
                    !matches!(e, syn::Expr::Binary(b) if expressions::comparison(&b.op))
                        && expressions::elapsed(e, &self.elapsed)
                }) {
                    self.record("clock-assertion");
                }
            }
        }
        if m.path.is_ident("assert_eq") || m.path.is_ident("assert_ne") {
            if let Ok(args) =
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated.parse2(m.tokens.clone())
            {
                if args
                    .iter()
                    .take(2)
                    .any(|e| expressions::elapsed(e, &self.elapsed) || expressions::clock(e, &BTreeSet::new()))
                {
                    self.record("clock-comparison");
                }
            }
        }
        expressions::macro_body(self, m.tokens.clone());
    }
    fn visit_item_macro(&mut self, item: &'a syn::ItemMacro) {
        let saved = self.test;
        self.test |= test(&item.attrs);
        self.visit_macro(&item.mac);
        self.test = saved;
    }
    fn visit_stmt_macro(&mut self, m: &'a syn::StmtMacro) {
        let saved = self.test;
        self.test |= test(&m.attrs);
        let clock_fork = self.clock_fork;
        self.clock_fork |= !saved && self.test;
        self.visit_macro(&m.mac);
        self.test = saved;
        self.clock_fork = clock_fork;
    }
}

fn exempt(path: &str, name: &str, kind: &str) -> bool {
    // Timestamp provenance compares clock origins without imposing a duration budget.
    if path == "ui/src/drawpipe.rs" && name == "tests::partial_merges_and_picture_age_are_counted" {
        return kind == "clock-comparison";
    }
    // These functions implement named hang guards, or are explicitly invoked manual observation tools.
    matches!(
        (path, name),
        ("ui/src/ui/lifecycle/test_wait.rs", "new" | "check" | "poll")
            | ("ui/tests/support/mod.rs", "run_owned")
            | ("hal/tests/support/mod.rs", "wait_for")
            | ("ui/src/http.rs", "tests::scpi_validation_fixture" | "tests::web_demo")
            | ("ui/src/mdns.rs", "tests::host_responder")
            | ("ui/src/ui/wlan.rs", "tests::live_radio")
            | ("ui/src/perfguard/deep/tests.rs", "bench_deep_diagnostics_overhead")
            | ("ui/src/sparse.rs", "tests::sparse_path_is_cheaper_than_the_record_path")
            | ("ui/src/trignear.rs", "tests::near_first_is_much_cheaper_at_the_hardware_point")
            | ("ui/src/perfguard/latency.rs", "check" | "wait_until")
    )
}
fn findings(source: &sources::Source) -> Ratchet {
    let mut walk = Walk { test: source.test, ..Default::default() };
    walk.visit_file(&source.file);
    Ratchet(
        walk.found
            .0
            .into_iter()
            .filter(|(k, _)| {
                let (name, kind) = k.rsplit_once(' ').unwrap_or((k, ""));
                !exempt(&source.rel, name, kind)
            })
            .map(|(k, v)| (format!("time {} {k}", source.rel), v))
            .collect(),
    )
}

pub(crate) fn current(root: &Path) -> Result<Vec<(&'static str, Ratchet)>, String> {
    let meta = workspace::load(&Default::default(), root)?;
    crate::repo::CRATES
        .iter()
        .map(|c| {
            let mut now = Ratchet::default();
            for source in sources::of_dir(&meta, c.dir)? {
                now.0.extend(findings(&source).0);
            }
            Ok((c.dir, now))
        })
        .collect()
}
pub(crate) fn check(ctx: &crate::run::Context, root: &Path) -> steps::Outcome {
    let now = current(root).map_err(|e| vec![e])?;
    steps::ratcheted(ctx, root, Kind::TestTime, &now)
}

#[cfg(test)]
mod tests;
