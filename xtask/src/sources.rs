//! The source files of a crate: every module reachable from its cargo targets, with its module path and whether it
//! is test code (a test target, or below a `#[cfg(test)]` module). `#[path]` on a module fails: the gate would have
//! to follow files anywhere.

use crate::workspace::{Metadata, Package};
use std::path::{Path, PathBuf};
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
    /// a file of a test, bench or example target (its `crate::` is that target)
    pub test_target: bool,
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

/// `#[cfg(test)]`.
fn cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs
        .iter()
        .any(|a| a.path().is_ident("cfg") && a.meta.require_list().is_ok_and(|l| l.tokens.to_string().trim() == "test"))
}

/// `#[path = ..]`, also inside `#[cfg_attr(.., path = ..)]`.
fn path_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        a.path().is_ident("path")
            || (a.path().is_ident("cfg_attr")
                && a.meta
                    .require_list()
                    .is_ok_and(|l| l.tokens.to_string().split(',').any(|p| p.trim().starts_with("path"))))
    })
}

/// A `mod x;` found in a file: the inline modules around it, its name, and whether it is test code.
struct Decl {
    inline: Vec<String>,
    name: String,
    test: bool,
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
}

impl<'a> Visit<'a> for Decls {
    fn visit_item_mod(&mut self, m: &'a ItemMod) {
        if path_attr(&m.attrs) {
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
            None => self.out.push(Decl { inline: self.inline.clone(), name: m.ident.to_string(), test }),
            Some(_) => {
                self.inline.push(m.ident.to_string());
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
fn decls(file: &syn::File, test: bool) -> Result<Vec<Decl>, String> {
    let mut d = Decls { test: vec![test], ..Decls::default() };
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
fn walk(root: &Path, start: &Path, test: bool, test_target: bool, out: &mut Vec<Source>) -> Result<(), String> {
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
        for d in decls(&file, test).map_err(|e| format!("{}: {}", rel, e))? {
            let base = d.inline.iter().fold(dir.clone(), |p, s| p.join(s));
            let (flat, nested) = (base.join(format!("{}.rs", d.name)), base.join(&d.name).join("mod.rs"));
            let child = match (flat.exists(), nested.exists()) {
                (true, _) => flat,
                (false, true) => nested,
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
        out.push(Source { rel, module, test, test_target, text, file });
    }
    Ok(())
}

/// Every source file of the package, from all its targets. A file reached from both a test target and the library
/// counts as library code.
pub fn of_package(meta: &Metadata, p: &Package) -> Result<Vec<Source>, String> {
    let root = Path::new(&meta.workspace_root);
    let mut targets: Vec<&crate::workspace::Target> = p.targets.iter().collect();
    // library and binaries first, so a shared file is library code
    targets.sort_by_key(|t| t.is_test());
    let mut out = Vec::new();
    for t in targets {
        walk(root, Path::new(&t.src_path), t.is_test(), t.is_test(), &mut out)?;
    }
    Ok(out)
}

/// The sources of the member in directory `dir`.
pub fn of_dir(meta: &Metadata, dir: &str) -> Result<Vec<Source>, String> {
    let p = meta.member(dir).ok_or_else(|| format!("{}: no workspace member", dir))?;
    of_package(meta, p)
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

    #[test]
    fn a_path_attribute_fails() {
        let root = fixture("path");
        std::fs::write(root.join("p").join("src").join("lib.rs"), "#[path = \"../x.rs\"]\nmod a;\n").expect("write");
        let e = of_dir(&meta(&root), "p").expect_err("must fail");
        std::fs::remove_dir_all(&root).expect("clean up");
        assert!(e.contains("#[path] is not allowed"), "{}", e);
    }
}
