//! Name resolution for the module rules: every path a file names, in every spelling the compiler accepts, as one
//! canonical path. Crate paths from the crate root (`board::dho`), dependency paths under the package name
//! (`oos_hal::dho`, also when Cargo renames it), raw identifiers as plain names.

use std::collections::{BTreeMap, BTreeSet};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::UseTree;

/// An identifier as the compiler resolves it: `r#dho` is `dho`.
pub(super) fn plain(i: &syn::Ident) -> String {
    i.unraw().to_string()
}

/// Test-only attributes: `cfg(test)` and `test`.
pub(super) fn test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("test")
            || (a.path().is_ident("cfg") && a.meta.require_list().is_ok_and(|l| l.tokens.to_string().trim() == "test"))
    })
}

/// One import of a use tree: its path as written, the name it binds, and whether it is a glob.
struct Import {
    path: Vec<String>,
    name: Option<String>,
    glob: bool,
}

/// The imports of a use tree under `prefix`: `a::x` and `b` for `{a::x, b}`; `self` names the prefix.
fn imports(t: &UseTree, prefix: &[String], out: &mut Vec<Import>) {
    let leaf = |i: &syn::Ident| {
        let mut p = prefix.to_vec();
        if i != "self" {
            p.push(plain(i));
        }
        p
    };
    match t {
        UseTree::Path(p) => {
            let mut next = prefix.to_vec();
            next.push(plain(&p.ident));
            imports(&p.tree, &next, out);
        }
        UseTree::Name(n) => {
            let path = leaf(&n.ident);
            out.push(Import { name: path.last().cloned(), path, glob: false });
        }
        UseTree::Rename(r) => out.push(Import { path: leaf(&r.ident), name: Some(plain(&r.rename)), glob: false }),
        UseTree::Group(g) => g.items.iter().for_each(|i| imports(i, prefix, out)),
        UseTree::Glob(_) => out.push(Import { path: prefix.to_vec(), name: None, glob: true }),
    }
}

/// The imports of a `use` item: its leading `::` as an empty first segment.
fn use_imports(u: &syn::ItemUse) -> Vec<Import> {
    let lead = if u.leading_colon.is_some() { vec![String::new()] } else { Vec::new() };
    let mut out = Vec::new();
    imports(&u.tree, &lead, &mut out);
    out
}

/// The names a module or a block has in scope besides the crate root and the dependencies: bound by `use` and
/// `extern crate ... as`, its child modules, and its glob imports. Paths as written there.
#[derive(Debug, Default)]
struct Scope {
    bindings: BTreeMap<String, Vec<String>>,
    children: BTreeSet<String>,
    globs: Vec<Vec<String>>,
}

impl Scope {
    fn add_use(&mut self, u: &syn::ItemUse) {
        for i in use_imports(u) {
            match (i.glob, i.name) {
                (true, _) => self.globs.push(i.path),
                (false, Some(n)) if n != "_" => {
                    self.bindings.insert(n, i.path);
                }
                _ => {}
            }
        }
    }

    fn add_extern(&mut self, e: &syn::ItemExternCrate) {
        if let Some((_, alias)) = &e.rename {
            let target =
                if e.ident == "self" { vec!["crate".to_string()] } else { vec![String::new(), plain(&e.ident)] };
            self.bindings.insert(plain(alias), target);
        }
    }

    /// The scope of a block: the `use` items among its statements.
    fn of_block(b: &syn::Block) -> Scope {
        let mut s = Scope::default();
        for st in &b.stmts {
            match st {
                syn::Stmt::Item(syn::Item::Use(u)) => s.add_use(u),
                syn::Stmt::Item(syn::Item::ExternCrate(e)) => s.add_extern(e),
                _ => {}
            }
        }
        s
    }
}

/// The module scopes of the files of a crate, by module path (`a::b`), outside test code. Blocks are not modules:
/// their imports stay in them.
#[derive(Debug, Default)]
struct Scopes {
    here: Vec<String>,
    out: BTreeMap<String, Scope>,
}

impl Scopes {
    fn scope(&mut self) -> &mut Scope {
        self.out.entry(self.here.join("::")).or_default()
    }
}

impl<'a> Visit<'a> for Scopes {
    fn visit_item_use(&mut self, u: &'a syn::ItemUse) {
        self.scope().add_use(u);
    }

    fn visit_item_extern_crate(&mut self, e: &'a syn::ItemExternCrate) {
        self.scope().add_extern(e);
    }

    fn visit_item_mod(&mut self, m: &'a syn::ItemMod) {
        if test_only(&m.attrs) {
            return;
        }
        self.scope().children.insert(plain(&m.ident));
        self.here.push(plain(&m.ident));
        visit::visit_item_mod(self, m);
        self.here.pop();
    }

    fn visit_block(&mut self, _: &'a syn::Block) {}
}

/// How far bindings may lead to bindings before resolution stops (a cycle does not compile anyway).
const DEPTH: usize = 12;

fn joined(head: &[String], rest: &[String]) -> String {
    head.iter().chain(rest).cloned().collect::<Vec<_>>().join("::")
}

fn segs(path: &str) -> Vec<String> {
    path.split("::").filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// `segs` from the crate root: `crate::a`, `self::a` and `super::a` resolved against `here`; None for a path that
/// does not start there (a local name, another crate, a too deep `super`).
fn resolve(here: &[String], segs: &[String]) -> Option<Vec<String>> {
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
    Some(base)
}

/// The crate the rules check: its dependencies (name in code -> package name with `_`) and the scopes of all its
/// modules, so a path is followed through other modules' re-exports.
#[derive(Debug)]
pub struct Crate<'e> {
    deps: &'e BTreeMap<String, String>,
    scopes: BTreeMap<String, Scope>,
}

impl<'e> Crate<'e> {
    /// The crate of `files` (each with its module path), outside test code.
    pub fn new(deps: &'e BTreeMap<String, String>, files: &[(&syn::File, &str)]) -> Crate<'e> {
        let mut s = Scopes::default();
        for (f, m) in files {
            s.here = segs(m);
            s.visit_file(f);
        }
        Crate { deps, scopes: s.out }
    }

    /// The canonical paths `file`, the source of `module`, names outside test code.
    pub fn named(&self, file: &syn::File, module: &str) -> Vec<String> {
        let mut n = Named { krate: self, here: segs(module), blocks: Vec::new(), out: Vec::new() };
        n.visit_file(file);
        n.out.retain(|p| !p.is_empty());
        n.out
    }

    fn dep_path(&self, first: &str, rest: &[String]) -> Option<String> {
        self.deps.get(first).map(|d| joined(std::slice::from_ref(d), rest))
    }

    /// The canonical paths `segs` may name in module `here` inside `blocks` (innermost last): a crate path from the
    /// root or a dependency path. Several when glob imports could explain it; none for a local, std or unknown path.
    fn canonical(&self, here: &[String], blocks: &[&Scope], segs: &[String], depth: usize) -> Vec<String> {
        let Some((first, rest)) = segs.split_first() else { return Vec::new() };
        if depth > DEPTH {
            return Vec::new();
        }
        if first.is_empty() {
            return rest.split_first().and_then(|(c, r)| self.dep_path(c, r)).into_iter().collect();
        }
        if let Some(abs) = resolve(here, segs) {
            return self.absolute(&abs, depth + 1);
        }
        let module = self.scopes.get(&here.join("::"));
        let lexical: Vec<&Scope> = blocks.iter().rev().copied().chain(module).collect();
        for (i, s) in lexical.iter().enumerate() {
            if let Some(target) = s.bindings.get(first).filter(|t| t.first() != Some(first)) {
                let mut t = target.clone();
                t.extend(rest.iter().cloned());
                let outer = &blocks[..blocks.len().saturating_sub(i)];
                return self.canonical(here, outer, &t, depth + 1);
            }
        }
        if module.is_some_and(|s| s.children.contains(first)) {
            return self.absolute(&[here, segs].concat(), depth + 1);
        }
        if let Some(d) = self.dep_path(first, rest) {
            return vec![d];
        }
        let globs: Vec<&Vec<String>> = lexical.iter().flat_map(|s| s.globs.iter()).collect();
        globs
            .into_iter()
            .flat_map(|g| self.canonical(here, blocks, g, depth + 1))
            .flat_map(|b| self.through_glob(&b, segs, depth + 1))
            .collect()
    }

    /// A crate path from the root (`[board, hw, wave]`) with the modules on its way followed: a child module, a
    /// binding (a re-export, also in another module), a glob import. The rest is an item path as written.
    fn absolute(&self, path: &[String], depth: usize) -> Vec<String> {
        let mut m: Vec<String> = Vec::new();
        for (i, s) in path.iter().enumerate() {
            let Some(scope) = self.scopes.get(&m.join("::")) else { break };
            if scope.children.contains(s) {
                m.push(s.clone());
                continue;
            }
            let rest = &path[i..];
            let found = match scope.bindings.get(s) {
                Some(t) => self.canonical(&m, &[], &[t.as_slice(), &rest[1..]].concat(), depth + 1),
                None => scope
                    .globs
                    .iter()
                    .flat_map(|g| self.canonical(&m, &[], g, depth + 1))
                    .flat_map(|b| self.through_glob(&b, rest, depth + 1))
                    .collect(),
            };
            if !found.is_empty() {
                return found;
            }
            break;
        }
        vec![path.join("::")]
    }

    /// `segs` through a glob import of `base`: into a dependency always, into a crate module only where it has the
    /// first segment as a child or a binding.
    fn through_glob(&self, base: &str, segs: &[String], depth: usize) -> Vec<String> {
        let into_dep = base.split("::").next().is_some_and(|f| self.deps.values().any(|d| d == f));
        if into_dep {
            return vec![format!("{}::{}", base, segs.join("::"))];
        }
        let has = |s: &Scope| s.children.contains(&segs[0]) || s.bindings.contains_key(&segs[0]);
        if !self.scopes.get(base).is_some_and(has) {
            return Vec::new();
        }
        self.absolute(&[self::segs(base), segs.to_vec()].concat(), depth)
    }
}

/// The walk over one file: the module it is in (inline `mod` included) and the block scopes around it.
#[derive(Debug)]
struct Named<'k, 'e> {
    krate: &'k Crate<'e>,
    here: Vec<String>,
    blocks: Vec<Scope>,
    out: Vec<String>,
}

impl Named<'_, '_> {
    fn record(&mut self, segs: &[String]) {
        let blocks: Vec<&Scope> = self.blocks.iter().collect();
        let found = self.krate.canonical(&self.here, &blocks, segs, 0);
        self.out.extend(found);
    }
}

impl<'a> Visit<'a> for Named<'_, '_> {
    fn visit_path(&mut self, p: &'a syn::Path) {
        let mut segs: Vec<String> = p.segments.iter().map(|s| plain(&s.ident)).collect();
        if p.leading_colon.is_some() {
            segs.insert(0, String::new());
        }
        if segs.len() > 1 {
            self.record(&segs);
        }
        visit::visit_path(self, p);
    }

    fn visit_item_use(&mut self, u: &'a syn::ItemUse) {
        for i in use_imports(u) {
            self.record(&i.path);
        }
    }

    fn visit_block(&mut self, b: &'a syn::Block) {
        self.blocks.push(Scope::of_block(b));
        visit::visit_block(self, b);
        self.blocks.pop();
    }

    fn visit_item_mod(&mut self, m: &'a syn::ItemMod) {
        if test_only(&m.attrs) {
            return;
        }
        // a module does not see the blocks around it
        let blocks = std::mem::take(&mut self.blocks);
        self.here.push(plain(&m.ident));
        visit::visit_item_mod(self, m);
        self.here.pop();
        self.blocks = blocks;
    }

    fn visit_item_fn(&mut self, f: &'a syn::ItemFn) {
        if !test_only(&f.attrs) {
            visit::visit_item_fn(self, f);
        }
    }

    fn visit_impl_item_fn(&mut self, f: &'a syn::ImplItemFn) {
        if !test_only(&f.attrs) {
            visit::visit_impl_item_fn(self, f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> syn::File {
        syn::parse_file(src).expect("parses")
    }

    fn deps() -> BTreeMap<String, String> {
        ["oos_hal", "oos_dsp"].iter().map(|d| (d.to_string(), d.to_string())).collect()
    }

    /// The canonical paths of `module` in a crate of `files` (module, source) over stub modules: the root declares
    /// `board` and `ui`, `board` declares `dho`, `board::dho` declares `hw`. A file given replaces its stub.
    fn names_in(files: &[(&str, &str)], module: &str, deps: &BTreeMap<String, String>) -> Vec<String> {
        let stubs =
            [("", "pub mod board;\npub mod ui;\n"), ("board", "pub mod dho;\n"), ("board::dho", "pub mod hw;\n")];
        let mut all: BTreeMap<&str, syn::File> = stubs.iter().map(|(m, s)| (*m, parse(s))).collect();
        all.extend(files.iter().map(|(m, s)| (*m, parse(s))));
        let list: Vec<(&syn::File, &str)> = all.iter().map(|(m, f)| (f, *m)).collect();
        let c = Crate::new(deps, &list);
        let mut n = c.named(&all[module], module);
        n.sort();
        n.dedup();
        n
    }

    fn names_with(src: &str, module: &str, deps: &BTreeMap<String, String>) -> Vec<String> {
        names_in(&[(module, src)], module, deps)
    }

    fn names(src: &str, module: &str) -> Vec<String> {
        names_with(src, module, &deps())
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
                   mod inner { use super::*; fn t() { super::h(); } }\n";
        let n = names(src, "board::dho::hw");
        assert_eq!(
            n,
            ["board::dho::hw", "board::dho::hw::h", "board::dho::hw::kid", "board::dho::sib::X", "board::top::g"]
        );
        assert!(names("fn f() { super::x(); }", "").is_empty());
    }

    // The rule G1 adds: paths into dependencies count as written, `::oos_hal` too; test code does not count.
    #[test]
    fn dependency_paths_count_and_test_code_does_not() {
        let src = "use oos_hal::dho::seq;\nfn f() { ::oos_hal::dho::afg::x(); std::mem::drop(1); }\n\
                   #[cfg(test)]\nmod tests { use oos_hal::dho::tpu; }\n#[test]\nfn t() { crate::board::dho::y(); }\n";
        assert_eq!(names(src, "ui"), ["oos_hal::dho::afg::x", "oos_hal::dho::seq"]);
    }

    // The hole: with `hal = { package = "oos-hal" }` the code says `hal::dho`, which the rule `oos_hal::dho` missed.
    #[test]
    fn a_renamed_dependency_counts_under_its_package_name() {
        let renamed: BTreeMap<String, String> = [("hal".to_string(), "oos_hal".to_string())].into();
        let src = "use hal::dho::seq;\nfn f() { hal::dho::afg::x(); ::hal::dho::tpu::y(); oos_hal::z(); }\n";
        assert_eq!(
            names_with(src, "ui", &renamed),
            ["oos_hal::dho::afg::x", "oos_hal::dho::seq", "oos_hal::dho::tpu::y"]
        );
    }

    // The hole: `r#dho` is `dho` to the compiler but not to the rules.
    #[test]
    fn raw_identifiers_are_their_plain_names() {
        let src = "use oos_hal::{r#dho::seq};\nuse r#oos_hal::dho::tpu;\n\
                   fn f() { oos_hal::r#dho::afg::x(); crate::r#board::r#dho::y(); }\n\
                   mod r#inner { fn g() { super::r#board::z(); } }\n";
        assert_eq!(
            names(src, "ui"),
            ["board::dho::y", "oos_hal::dho::afg::x", "oos_hal::dho::seq", "oos_hal::dho::tpu", "ui::board::z"]
        );
    }

    // The holes: a name brought in by `use`, `extern crate` or a glob, or a child module named from its parent, spelled
    // the confined path without writing it.
    #[test]
    fn imports_child_modules_and_globs_resolve_to_the_path_they_name() {
        for (module, src, want) in [
            ("ui", "use oos_hal as h;\nfn f() { h::dho::x(); }\n", "oos_hal::dho::x"),
            ("ui", "use oos_hal::{self as h};\nfn f() { h::dho::x(); }\n", "oos_hal::dho::x"),
            ("ui", "extern crate oos_hal as e;\nfn f() { e::dho::x(); }\n", "oos_hal::dho::x"),
            ("ui", "use oos_hal::*;\nfn f() { dho::x(); }\n", "oos_hal::dho::x"),
            ("ui", "use crate::board;\nfn f() { board::dho::y(); }\n", "board::dho::y"),
            ("ui", "use super::board as b;\nfn f() { b::dho::y(); }\n", "board::dho::y"),
            ("ui", "use crate::board::*;\nfn f() { dho::y(); }\n", "board::dho::y"),
            ("ui", "extern crate self as me;\nfn f() { me::board::dho::y(); }\n", "board::dho::y"),
            ("", "mod board;\nfn f() { board::dho::y(); }\n", "board::dho::y"),
            ("ui", "mod m { use crate::board as b; fn f() { b::dho::y(); } }\n", "board::dho::y"),
        ] {
            assert!(names(src, module).iter().any(|n| n == want), "{}: {:?}", src, names(src, module));
        }
        // a binding is scoped to its module, and a glob explains only modules that exist
        assert!(!names("mod m { use crate::board as b; }\nfn f() { b::dho::y(); }\n", "ui")
            .iter()
            .any(|n| n.contains("dho")));
        assert_eq!(names("use crate::board::*;\nfn f() { Vec::new(); }\n", "ui"), ["board"]);
    }

    // The hole: `self::` and `crate::` paths skipped the module's bindings, so `self::board::dho` after
    // `use crate::board;` resolved to `ui::board::dho`.
    #[test]
    fn qualified_paths_go_through_bindings() {
        for (src, want) in [
            ("use crate::board;\nfn f() { self::board::dho::wave(); }\n", "board::dho::wave"),
            ("use oos_hal as h;\nfn f() { self::h::dho::x(); }\n", "oos_hal::dho::x"),
            ("use oos_hal as h;\nfn f() { crate::ui::h::dho::x(); }\n", "oos_hal::dho::x"),
            ("use crate::board;\nmod m { fn f() { super::board::dho::wave(); } }\n", "board::dho::wave"),
        ] {
            assert!(names(src, "ui").iter().any(|n| n == want), "{}: {:?}", src, names(src, "ui"));
        }
    }

    // The hole: a re-export in another module (`pub use self::dho as hw;` in board) hid the confined path, directly
    // and through a glob, also over several modules.
    #[test]
    fn re_exports_are_followed_across_modules() {
        let board = ("board", "pub mod dho;\npub use self::dho as hw;\n");
        let root = ("", "pub mod board;\npub mod ui;\npub mod deep;\n");
        let deep = ("deep", "pub use crate::board::hw as again;\n");
        for (src, want) in [
            ("fn f() { crate::board::hw::wave(); }\n", "board::dho::wave"),
            ("use crate::board::*;\nfn f() { hw::wave(); }\n", "board::dho::wave"),
            ("use crate::board::hw;\nfn f() { hw::wave(); }\n", "board::dho::wave"),
            ("fn f() { crate::deep::again::wave(); }\n", "board::dho::wave"),
        ] {
            let n = names_in(&[board, root, deep, ("ui", src)], "ui", &deps());
            assert!(n.iter().any(|x| x == want), "{}: {:?}", src, n);
        }
    }

    // The hole: a `use` inside a function counted for the whole module, so it replaced the module's binding of the
    // same name: a false finding in one direction, a hidden one in the other.
    #[test]
    fn imports_in_blocks_count_only_in_their_block() {
        let src = "fn a() { use crate::safe as h; h::dho(); }\nfn b() { use oos_hal as h; h::crc32(); }\n";
        assert_eq!(names(src, "ui"), ["oos_hal", "oos_hal::crc32", "safe", "safe::dho"]);
        let src = "use crate::safe as h;\nfn a() { h::dho::x(); }\nfn b() { use oos_hal as h; h::dho::y(); }\n";
        assert_eq!(names(src, "ui"), ["oos_hal", "oos_hal::dho::y", "safe", "safe::dho::x"]);
        let src = "use oos_hal as h;\nfn a() { h::dho::x(); }\nfn b() { use crate::safe as h; h::dho::y(); }\n";
        assert_eq!(names(src, "ui"), ["oos_hal", "oos_hal::dho::x", "safe", "safe::dho::y"]);
        // inner blocks see the outer ones
        let src = "fn a() { use oos_hal as h; { h::dho::x(); } }\n";
        assert_eq!(names(src, "ui"), ["oos_hal", "oos_hal::dho::x"]);
    }
}
