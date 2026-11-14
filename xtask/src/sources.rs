//! The source files of a crate: every module reachable from its cargo targets, with its module path and whether it
//! is test code (a test target, or below a `#[cfg(test)]` module). `#[path]` on a module fails: the gate would have
//! to follow files anywhere.

use crate::workspace::{Metadata, Package};
use std::path::{Path, PathBuf};
use syn::{Item, ItemMod};

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
fn cfg_test(m: &ItemMod) -> bool {
    m.attrs
        .iter()
        .any(|a| a.path().is_ident("cfg") && a.meta.require_list().is_ok_and(|l| l.tokens.to_string().trim() == "test"))
}

/// A `mod x;` found in a file: the inline modules around it, its name, and whether it is test code.
struct Decl {
    inline: Vec<String>,
    name: String,
    test: bool,
}

/// The `mod x;` declarations in `items`, inside inline modules too.
fn decls(items: &[Item], inline: &[String], test: bool, out: &mut Vec<Decl>) -> Result<(), String> {
    for it in items {
        let Item::Mod(m) = it else { continue };
        if m.attrs.iter().any(|a| a.path().is_ident("path")) {
            return Err(format!("mod {}: #[path] is not allowed (the gate follows the default module files)", m.ident));
        }
        let t = test || cfg_test(m);
        match &m.content {
            None => out.push(Decl { inline: inline.to_vec(), name: m.ident.to_string(), test: t }),
            Some((_, inner)) => {
                let mut path = inline.to_vec();
                path.push(m.ident.to_string());
                decls(inner, &path, t, out)?;
            }
        }
    }
    Ok(())
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

/// Walks the module graph from one target root.
fn walk(root: &Path, start: &Path, test: bool, test_target: bool, out: &mut Vec<Source>) -> Result<(), String> {
    let mut todo = vec![(start.to_path_buf(), String::new(), test, true)];
    while let Some((path, module, test, is_root)) = todo.pop() {
        let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        if out.iter().any(|s| s.rel == rel) {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", rel, e))?;
        let file = syn::parse_file(&text).map_err(|e| format!("{}: line {}: {}", rel, e.span().start().line, e))?;
        let mut found = Vec::new();
        decls(&file.items, &[], test, &mut found).map_err(|e| format!("{}: {}", rel, e))?;
        let dir = child_dir(&path, is_root);
        for d in found {
            let base = d.inline.iter().fold(dir.clone(), |p, s| p.join(s));
            let (flat, nested) = (base.join(format!("{}.rs", d.name)), base.join(&d.name).join("mod.rs"));
            let child = if flat.exists() {
                flat
            } else if nested.exists() {
                nested
            } else {
                return Err(format!("{}: mod {}: no {} or {}", rel, d.name, flat.display(), nested.display()));
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

    #[test]
    fn a_path_attribute_fails() {
        let root = fixture("path");
        std::fs::write(root.join("p").join("src").join("lib.rs"), "#[path = \"../x.rs\"]\nmod a;\n").expect("write");
        let e = of_dir(&meta(&root), "p").expect_err("must fail");
        std::fs::remove_dir_all(&root).expect("clean up");
        assert!(e.contains("#[path] is not allowed"), "{}", e);
    }
}
