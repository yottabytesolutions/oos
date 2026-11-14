//! Module rules inside a crate: which modules a module may name through `crate::`. Every path counts (`use` trees,
//! inline calls, types); a rule names a module path (`board::dho`), and the most specific rule owns a file.

use super::rules::{Forbid, Layer};
use crate::sources::Source;
use syn::visit::{self, Visit};
use syn::UseTree;

/// The module paths a file names, resolved from the crate root, as `a::b::c`. `here` is the module the walk is in:
/// the file's module, then any inline `mod` inside it.
#[derive(Debug, Default)]
struct Named {
    here: Vec<String>,
    out: Vec<String>,
}

/// `segs` from the crate root: `crate::a`, `self::a` and `super::a` resolved against `here`; None for a path that
/// does not start there (a local name, another crate, a too deep `super`).
fn resolve(here: &[String], segs: &[String]) -> Option<String> {
    let mut base: Vec<String> = match segs.first().map(String::as_str) {
        Some("crate") => Vec::new(),
        Some("self" | "super") => here.to_vec(),
        _ => return None,
    };
    let mut rest = &segs[1..];
    if segs[0] == "super" {
        base.pop()?;
    }
    while rest.first().is_some_and(|s| s == "super") {
        base.pop()?;
        rest = &rest[1..];
    }
    base.extend(rest.iter().cloned());
    Some(base.join("::"))
}

impl<'a> Visit<'a> for Named {
    fn visit_path(&mut self, p: &'a syn::Path) {
        let segs: Vec<String> = p.segments.iter().map(|s| s.ident.to_string()).collect();
        if segs.len() > 1 {
            self.out.extend(resolve(&self.here, &segs));
        }
        visit::visit_path(self, p);
    }

    fn visit_item_use(&mut self, u: &'a syn::ItemUse) {
        let mut raw = Vec::new();
        use_paths(&u.tree, "", &mut raw);
        for path in raw {
            let segs: Vec<String> = path.split("::").map(str::to_string).collect();
            self.out.extend(resolve(&self.here, &segs));
        }
    }

    fn visit_item_mod(&mut self, m: &'a syn::ItemMod) {
        self.here.push(m.ident.to_string());
        visit::visit_item_mod(self, m);
        self.here.pop();
    }
}

/// The paths of a use tree under `prefix`: `a::x` and `b` for `{a::x, b}`.
fn use_paths(t: &UseTree, prefix: &str, out: &mut Vec<String>) {
    let join = |name: &str| if prefix.is_empty() { name.to_string() } else { format!("{}::{}", prefix, name) };
    match t {
        UseTree::Path(p) => use_paths(&p.tree, &join(&p.ident.to_string()), out),
        UseTree::Name(n) if n.ident == "self" => out.push(prefix.to_string()),
        UseTree::Name(n) => out.push(join(&n.ident.to_string())),
        UseTree::Rename(r) => out.push(join(&r.ident.to_string())),
        UseTree::Group(g) => g.items.iter().for_each(|i| use_paths(i, prefix, out)),
        UseTree::Glob(_) => out.push(prefix.to_string()),
    }
}

/// The module paths `file`, the source of module `module` (`a::b`, empty for the crate root), names in its crate.
pub fn named(file: &syn::File, module: &str) -> Vec<String> {
    let here = module.split("::").filter(|s| !s.is_empty()).map(str::to_string).collect();
    let mut n = Named { here, out: Vec::new() };
    n.visit_file(file);
    n.out.retain(|p| !p.is_empty());
    n.out
}

/// `path` is `module` or inside it, segment-wise (`board::dho::hw` is in `board`, `boarding` is not).
pub fn within(path: &str, module: &str) -> bool {
    path == module || path.starts_with(&format!("{}::", module))
}

/// Violations of the `forbid` and `layer` rules in `sources` (the files of one crate). Test targets are other
/// crates (their `crate::` is the test) and are left out.
pub fn check(sources: &[Source], forbid: &[&Forbid], layers: &[&Layer]) -> Vec<String> {
    let mut out = Vec::new();
    for s in sources.iter().filter(|s| !s.test_target) {
        let (rel, module, names) = (&s.rel, &s.module, named(&s.file, &s.module));
        for f in forbid.iter().filter(|f| within(module, &f.from)) {
            if let Some(n) = names.iter().find(|n| within(n, &f.to)) {
                out.push(format!("{} names crate::{} (forbid {} -> {})", rel, n, f.from, f.to));
            }
        }
        // the most specific layer owns the file
        let Some(l) = layers.iter().filter(|l| within(module, &l.module)).max_by_key(|l| l.module.len()) else {
            continue;
        };
        let bad = names.iter().filter(|n| !within(n, &l.module) && !l.allowed.iter().any(|a| within(n, a)));
        let mut bad: Vec<&String> = bad.collect();
        bad.sort();
        bad.dedup();
        out.extend(
            bad.into_iter().map(|n| format!("{} names crate::{} (layer {} allows {:?})", rel, n, l.module, l.allowed)),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> syn::File {
        syn::parse_file(src).expect("parses")
    }

    fn names(src: &str, module: &str) -> Vec<String> {
        let mut n = named(&parse(src), module);
        n.sort();
        n.dedup();
        n
    }

    #[test]
    fn finds_paths_and_use_trees() {
        let src = "use crate::{model::Model, wave as w, a::{b, c::*}};\nuse std::fmt;\n\
                   fn f() -> crate::draw::T { crate::draw::go(); self::x() }\n";
        assert_eq!(names(src, ""), ["a::b", "a::c", "draw::T", "draw::go", "model::Model", "wave", "x"]);
    }

    #[test]
    fn self_and_super_resolve_against_the_module_and_inline_mods() {
        let src = "use super::sib::X;\nuse self::kid;\nfn f() { super::super::top::g(); }\n\
                   mod tests { use super::*; fn t() { super::h(); } }\n";
        let n = names(src, "board::dho::hw");
        assert_eq!(
            n,
            ["board::dho::hw", "board::dho::hw::h", "board::dho::hw::kid", "board::dho::sib::X", "board::top::g"]
        );
        assert!(names("fn f() { super::x(); }", "").is_empty());
        assert!(within("board::dho::hw", "board") && within("board", "board") && !within("boarding", "board"));
    }

    fn source(rel: &str, module: &str, src: &str, test_target: bool) -> Source {
        Source {
            rel: rel.into(),
            module: module.into(),
            test: test_target,
            test_target,
            text: src.into(),
            file: parse(src),
        }
    }

    #[test]
    fn forbid_and_the_most_specific_layer_decide() {
        let sources = [
            source(
                "ui/src/render.rs",
                "render",
                "use crate::model::M;\nfn f() { crate::wave::w(); crate::render::gl::x(); }\n",
                false,
            ),
            source(
                "ui/src/render/sub.rs",
                "render::sub",
                "fn f() { crate::input::k(); super::super::wave::w(); }\n",
                false,
            ),
            source(
                "ui/src/render/gl/mod.rs",
                "render::gl",
                "fn f() { crate::input::k(); crate::model::m(); }\n",
                false,
            ),
            // an integration test's crate:: is the test itself
            source("ui/tests/t.rs", "", "fn f() { crate::wave::w(); }\n", true),
        ];
        let set = |m: &[&str]| m.iter().map(|x| x.to_string()).collect();
        let f = Forbid { krate: "ui".into(), from: "render".into(), to: "wave".into() };
        let outer = Layer { krate: "ui".into(), module: "render".into(), allowed: set(&["model", "wave"]) };
        let inner = Layer { krate: "ui".into(), module: "render::gl".into(), allowed: set(&["input"]) };
        let v = check(&sources, &[&f], &[&outer, &inner]);
        assert_eq!(v.len(), 4, "{:#?}", v);
        assert!(v[0].starts_with("ui/src/render.rs names crate::wave::w (forbid render -> wave)"));
        // `super::super::wave` from render::sub is crate::wave
        assert!(v[1].starts_with("ui/src/render/sub.rs names crate::wave::w (forbid render -> wave)"));
        assert!(v[2].starts_with("ui/src/render/sub.rs names crate::input::k (layer render "));
        assert!(v[3].starts_with("ui/src/render/gl/mod.rs names crate::model::m (layer render::gl"));
    }
}
