//! The source files of a crate: every module reachable from its cargo targets, with its module path and whether it
//! is test code (a test target, or below a `#[cfg(test)]` module). `#[path]` on a module fails: the gate would have
//! to follow files anywhere. Only `cargo xtask loc` follows a plain `#[path = ".."]`, to measure old releases; it
//! takes the default file where `cfg_attr` sets a path.

use crate::workspace::{Metadata, Package};
use std::path::{Path, PathBuf};
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::ItemMod;

/// One source file of a crate.
pub struct Source {
    /// path relative to the workspace root
    pub rel: String,
    /// module path in its target (`board::dho::hw`, empty for a target's root file)
    pub module: String,
    /// test code: a test target or below `#[cfg(test)]`
    pub test: bool,
    pub text: String,
    pub file: syn::File,
}

impl std::fmt::Debug for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Source")
            .field("rel", &self.rel)
            .field("module", &self.module)
            .field("test", &self.test)
            .finish()
    }
}

/// A cfg predicate that cannot be true without `test`.
fn requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) => {
            use syn::parse::Parser;
            let Ok(items) =
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated.parse2(list.tokens.clone())
            else {
                return false;
            };
            if list.path.is_ident("all") {
                items.iter().any(requires_test)
            } else if list.path.is_ident("any") {
                !items.is_empty() && items.iter().all(requires_test)
            } else {
                false
            }
        }
        _ => false,
    }
}
/// `#[cfg(test)]`, including compound predicates that require test mode.
pub(crate) fn cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("cfg") && a.parse_args::<syn::Meta>().is_ok_and(|m| requires_test(&m)))
}

/// `#[path = ..]`, also inside `cfg_attr` (`crate::attrs`).
fn path_attr(attrs: &[syn::Attribute]) -> bool {
    crate::attrs::any(attrs, &["path"])
}

/// The file of a plain `#[path = "file"]`.
fn path_value(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().filter(|a| a.path().is_ident("path")).find_map(|a| match &a.meta.require_name_value().ok()?.value {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(s), .. }) => Some(s.value()),
        _ => None,
    })
}

/// A `mod x;` found in a file: the inline modules around it, its name, whether it is test code, and its
/// `#[path]` when followed.
struct Decl {
    inline: Vec<String>,
    name: String,
    test: bool,
    path: Option<String>,
}

/// Every `mod` in a file, in function bodies and impls too: the `mod x;` declarations to follow, or why the file is
/// refused (`#[path]`, a `mod x;` inside a function body).
#[derive(Default)]
struct Decls {
    inline: Vec<String>,
    test: Vec<bool>,
    depth: usize,
    out: Vec<Decl>,
    err: Option<String>,
    /// follow a plain `#[path]` and ignore one in `cfg_attr` instead of refusing them
    follow: bool,
}

impl<'a> Visit<'a> for Decls {
    fn visit_item_mod(&mut self, m: &'a ItemMod) {
        let path = if self.follow { path_value(&m.attrs) } else { None };
        if path_attr(&m.attrs) && !self.follow {
            self.err.get_or_insert(format!(
                "mod {}: #[path] is not allowed (the gate follows the default module files)",
                m.ident
            ));
        }
        let test = self.test.last().copied().unwrap_or(false) || cfg_test(&m.attrs);
        match &m.content {
            None if self.depth > 0 => {
                self.err.get_or_insert(format!("mod {}; inside a function body", m.ident));
            }
            None => self.out.push(Decl { inline: self.inline.clone(), name: m.ident.unraw().to_string(), test, path }),
            Some(_) => {
                self.inline.push(m.ident.unraw().to_string());
                self.test.push(test);
                visit::visit_item_mod(self, m);
                self.test.pop();
                self.inline.pop();
            }
        }
    }

    fn visit_block(&mut self, b: &'a syn::Block) {
        self.depth += 1;
        visit::visit_block(self, b);
        self.depth -= 1;
    }
}

/// The `mod x;` declarations of `file`; `test` is the file's own context.
fn decls(file: &syn::File, test: bool, follow: bool) -> Result<Vec<Decl>, String> {
    let mut d = Decls { test: vec![test], follow, ..Decls::default() };
    d.visit_file(file);
    match d.err {
        Some(e) => Err(e),
        None => Ok(d.out),
    }
}

/// The directory a file's child modules live in: its own for `lib.rs`, `main.rs`, `mod.rs` and target roots,
/// `<dir>/<stem>` otherwise.
fn child_dir(path: &Path, is_root: bool) -> PathBuf {
    let dir = path.parent().unwrap_or(path).to_path_buf();
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if is_root || stem == "mod" {
        dir
    } else {
        dir.join(stem)
    }
}

/// Walks the module graph from one target root. A file reached as test code and as production code is production
/// code: its children are walked again in that context.
fn walk(root: &Path, start: &Path, test: bool, follow: bool, out: &mut Vec<Source>) -> Result<(), String> {
    let mut todo = vec![(start.to_path_buf(), String::new(), test, true)];
    while let Some((path, module, test, is_root)) = todo.pop() {
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        match out.iter().position(|s| s.rel == rel) {
            Some(i) if out[i].test && !test => {
                out.remove(i);
            }
            Some(_) => continue,
            None => {}
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", rel, e))?;
        let file = syn::parse_file(&text).map_err(|e| format!("{}: line {}: {}", rel, e.span().start().line, e))?;
        let dir = child_dir(&path, is_root);
        for d in decls(&file, test, follow).map_err(|e| format!("{}: {}", rel, e))? {
            let base = d.inline.iter().fold(dir.clone(), |p, s| p.join(s));
            let (flat, nested) = (base.join(format!("{}.rs", d.name)), base.join(&d.name).join("mod.rs"));
            let child = match (&d.path, flat.exists(), nested.exists()) {
                // relative to the file's own directory, or to the inline modules' directory
                (Some(p), ..) if d.inline.is_empty() => path.parent().unwrap_or(root).join(p),
                (Some(p), ..) => base.join(p),
                (None, true, _) => flat,
                (None, false, true) => nested,
                _ => return Err(format!("{}: mod {}: no {} or {}", rel, d.name, flat.display(), nested.display())),
            };
            let segs: Vec<&str> = [module.as_str()]
                .into_iter()
                .chain(d.inline.iter().map(String::as_str))
                .chain([d.name.as_str()])
                .filter(|s| !s.is_empty())
                .collect();
            todo.push((child, segs.join("::"), d.test, false));
        }
        out.push(Source { rel, module, test, text, file });
    }
    Ok(())
}

/// Every source file of the package, from all its targets. A file reached from both a test target and the library
/// counts as library code. `follow`: follow a plain `#[path]` instead of failing on it.
pub fn of_package(meta: &Metadata, p: &Package, follow: bool) -> Result<Vec<Source>, String> {
    let root = Path::new(&meta.workspace_root);
    let mut targets: Vec<&crate::workspace::Target> = p.targets.iter().collect();
    // library and binaries first, so a shared file is library code
    targets.sort_by_key(|t| t.is_test());
    let mut out = Vec::new();
    for t in targets {
        walk(root, Path::new(&t.src_path), t.is_test(), follow, &mut out)?;
    }
    Ok(out)
}

/// The sources of the member in directory `dir`.
pub fn of_dir(meta: &Metadata, dir: &str) -> Result<Vec<Source>, String> {
    let p = meta.member(dir).ok_or_else(|| format!("{}: no workspace member", dir))?;
    of_package(meta, p, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A package under a fresh temporary root: lib.rs with `mod a;` (a.rs with `mod b;` in a/b.rs), a test module
    /// in its own file, and an integration test.
    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("xtask-sources-{}-{}", name, std::process::id()));
        let src = root.join("p").join("src");
        std::fs::create_dir_all(src.join("a")).expect("dir");
        std::fs::create_dir_all(root.join("p").join("tests")).expect("dir");
        let w = |p: PathBuf, t: &str| std::fs::write(p, t).expect("write");
        w(src.join("lib.rs"), "mod a;\n#[cfg(test)]\nmod tests;\n");
        w(src.join("a.rs"), "mod b;\nmod inner { mod c; }\n");
        std::fs::create_dir_all(src.join("a").join("inner")).expect("dir");
        w(src.join("a").join("b.rs"), "fn f() {}\n");
        w(src.join("a").join("inner").join("c.rs"), "fn g() {}\n");
        w(src.join("tests.rs"), "fn t() {}\n");
        w(root.join("p").join("tests").join("it.rs"), "fn i() {}\n");
        root
    }

    fn meta(root: &Path) -> Metadata {
        let r = root.display();
        crate::workspace::parse(&format!(
            r#"{{"workspace_root":"{r}","packages":[{{"name":"p","manifest_path":"{r}/p/Cargo.toml","dependencies":[],"targets":[
                {{"kind":["lib"],"src_path":"{r}/p/src/lib.rs"}},{{"kind":["test"],"src_path":"{r}/p/tests/it.rs"}}]}}]}}"#
        ))
        .expect("metadata")
    }

    #[test]
    fn follows_modules_and_carries_test_context() {
        let root = fixture("graph");
        let mut s = of_dir(&meta(&root), "p").expect("sources");
        std::fs::remove_dir_all(&root).expect("clean up");
        s.sort_by(|a, b| a.rel.cmp(&b.rel));
        let got: Vec<(&str, &str, bool)> = s.iter().map(|x| (x.rel.as_str(), x.module.as_str(), x.test)).collect();
        assert_eq!(
            got,
            [
                ("p/src/a.rs", "a", false),
                ("p/src/a/b.rs", "a::b", false),
                ("p/src/a/inner/c.rs", "a::inner::c", false),
                ("p/src/lib.rs", "", false),
                ("p/src/tests.rs", "tests", true),
                ("p/tests/it.rs", "", true),
            ]
        );
    }

    // The hole: the same file declared under cfg(test) and cfg(not(test)) kept whichever context came first.
    #[test]
    fn production_context_wins_over_test() {
        let root = fixture("twice");
        let src = root.join("p").join("src");
        std::fs::write(src.join("lib.rs"), "#[cfg(test)]\nmod a;\n#[cfg(not(test))]\nmod a;\n").expect("write");
        let s = of_dir(&meta(&root), "p").expect("sources");
        std::fs::remove_dir_all(&root).expect("clean up");
        let a: Vec<bool> = s.iter().filter(|x| x.rel.starts_with("p/src/a")).map(|x| x.test).collect();
        assert_eq!(a, [false, false, false], "{:?}", s);
    }

    // The holes: a module declared in a function body, or with #[path] inside cfg_attr, escaped the walk.
    #[test]
    fn modules_in_function_bodies_and_cfg_attr_paths_fail() {
        for (lib, why) in [
            ("fn f() {\n    #[path = \"x.rs\"]\n    mod hidden;\n}\n", "#[path]"),
            ("fn f() { mod hidden; }\n", "inside a function body"),
            ("impl S { fn f() { mod inner { mod deep; } } }\n", "inside a function body"),
            ("#[cfg_attr(all(), path = \"../x.rs\")]\nmod a;\n", "#[path]"),
        ] {
            let root = fixture("body");
            std::fs::write(root.join("p").join("src").join("lib.rs"), lib).expect("write");
            let e = of_dir(&meta(&root), "p").expect_err(lib);
            std::fs::remove_dir_all(&root).expect("clean up");
            assert!(e.contains(why), "{}: {}", lib, e);
        }
    }

    // `mod r#a;` is the module `a` in a.rs; the walk looked for `r#a.rs` and its module path kept the `r#`.
    #[test]
    fn a_raw_module_name_is_its_plain_name() {
        let root = fixture("raw");
        std::fs::write(root.join("p").join("src").join("lib.rs"), "mod r#a;\nmod outer { mod r#inner {} }\n")
            .expect("write");
        let s = of_dir(&meta(&root), "p").expect("sources");
        std::fs::remove_dir_all(&root).expect("clean up");
        assert!(s.iter().any(|x| x.rel == "p/src/a.rs" && x.module == "a"), "{:?}", s);
        assert!(s.iter().any(|x| x.rel == "p/src/a/inner/c.rs" && x.module == "a::inner::c"), "{:?}", s);
    }

    #[test]
    fn loc_follows_a_plain_path_attribute() {
        let root = fixture("follow");
        let src = root.join("p").join("src");
        std::fs::write(src.join("lib.rs"), "#[path = \"other.rs\"]\nmod a;\n").expect("write");
        std::fs::write(src.join("a.rs"), "#[path = \"x.rs\"]\nmod b;\n").expect("write");
        std::fs::write(src.join("other.rs"), "fn o() {}\n").expect("write");
        let m = meta(&root);
        let s = of_package(&m, m.member("p").expect("member"), true).expect("sources");
        std::fs::remove_dir_all(&root).expect("clean up");
        let rels: Vec<&str> = s.iter().filter(|x| !x.test).map(|x| x.rel.as_str()).collect();
        assert_eq!(rels, ["p/src/lib.rs", "p/src/other.rs"], "{:?}", s);
    }

    #[test]
    fn a_path_attribute_fails() {
        let root = fixture("path");
        std::fs::write(root.join("p").join("src").join("lib.rs"), "#[path = \"../x.rs\"]\nmod a;\n").expect("write");
        let e = of_dir(&meta(&root), "p").expect_err("must fail");
        std::fs::remove_dir_all(&root).expect("clean up");
        assert!(e.contains("#[path] is not allowed"), "{}", e);
    }
}

#[cfg(test)]
mod compound_test_cfg {
    use super::*;
    #[test]
    fn test_context_follows_external_compound_cfg_modules() {
        let file =
            syn::parse_file("#[cfg(all(test, unix))] mod only_tests; #[cfg(any(test, unix))] mod production;").unwrap();
        let found = decls(&file, false, false).unwrap();
        assert!(found[0].test);
        assert!(!found[1].test);
    }
}
