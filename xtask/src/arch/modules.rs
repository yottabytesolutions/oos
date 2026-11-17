// SPDX-License-Identifier: GPL-2.0-or-later
//! Module rules inside a crate: which modules a module may name. Every path counts (`use` trees, inline calls,
//! types), resolved to one canonical form (`resolve`). The most specific `layer` rule owns a file. Test code is left
//! out: tests may look behind a boundary.

use super::resolve::Crate;
use super::rules::{Confine, Forbid, Layer};
use super::spelled;
use crate::sources::Source;
use std::collections::BTreeMap;

/// `path` is `module` or inside it, segment-wise (`board::dho::hw` is in `board`, `boarding` is not).
pub fn within(path: &str, module: &str) -> bool {
    path == module || path.starts_with(&format!("{}::", module))
}

/// The rules of one crate.
#[derive(Debug, Default)]
pub struct CrateRules<'r> {
    pub forbid: Vec<&'r Forbid>,
    pub layers: Vec<&'r Layer>,
    pub confines: Vec<&'r Confine>,
    /// the crate's dependencies: name in code -> package name with `_` (`hal` -> `oos_hal`)
    pub deps: BTreeMap<String, String>,
}

impl CrateRules<'_> {
    fn is_dep(&self, path: &str) -> bool {
        path.split("::").next().is_some_and(|f| self.deps.values().any(|d| d == f))
    }

    /// `crate::a::b` for a crate path, the path itself for a dependency path.
    fn show(&self, path: &str) -> String {
        if self.is_dep(path) {
            path.to_string()
        } else {
            format!("crate::{}", path)
        }
    }

    /// Violations of the `layer` rule that owns `module`: crate paths outside it and its allowed modules.
    fn layer(&self, rel: &str, module: &str, names: &[String], out: &mut Vec<String>) {
        let Some(l) = self.layers.iter().filter(|l| within(module, &l.module)).max_by_key(|l| l.module.len()) else {
            return;
        };
        let mut bad: Vec<&String> = names
            .iter()
            .filter(|n| !self.is_dep(n) && !within(n, &l.module) && !l.allowed.iter().any(|a| within(n, a)))
            .collect();
        bad.sort();
        bad.dedup();
        out.extend(
            bad.into_iter()
                .map(|n| format!("{} names {} (layer {} allows {:?})", rel, self.show(n), l.module, l.allowed)),
        );
    }

    /// Whether `c` lets code of `module` name its path: inside the path itself or inside an allowed module.
    fn confine_allows(c: &Confine, module: &str) -> bool {
        within(module, &c.path) || c.allowed.iter().any(|a| within(module, a))
    }

    /// The spellings of a confined path in code: as written, and with each name in code of its dependency.
    fn spellings(&self, path: &str) -> Vec<Vec<String>> {
        let segs: Vec<String> = path.split("::").map(str::to_string).collect();
        let mut out = vec![segs.clone()];
        for (code, canon) in &self.deps {
            if segs.first() == Some(canon) && code != canon {
                out.push(std::iter::once(code.clone()).chain(segs[1..].iter().cloned()).collect());
            }
        }
        out
    }

    /// Violations of the `confine` rules: a confined path named outside its modules (and itself); failing that, a
    /// path whose tokens spell it there (a macro body, which resolution cannot see).
    fn confine(&self, s: &Source, names: &[String], out: &mut Vec<String>) {
        let spelled = spelled::spelled(&s.file, &s.module);
        for c in &self.confines {
            if Self::confine_allows(c, &s.module) {
                continue;
            }
            if let Some(n) = names.iter().find(|n| within(n, &c.path)) {
                out.push(format!("{} names {} (confine {} -> {:?})", s.rel, self.show(n), c.path, c.allowed));
                continue;
            }
            let ways = self.spellings(&c.path);
            let hit = spelled.iter().find(|(m, run)| {
                !Self::confine_allows(c, m)
                    && ways.iter().any(|w| {
                        let w: Vec<&str> = w.iter().map(String::as_str).collect();
                        spelled::spells(run, &w)
                    })
            });
            if let Some((_, run)) = hit {
                out.push(format!(
                    "{} spells {} in its tokens, a macro body for example (confine {} -> {:?})",
                    s.rel,
                    run.join("::"),
                    c.path,
                    c.allowed
                ));
            }
        }
    }
}

/// Violations of the module rules in `sources` (the files of one crate). Test code is left out: test targets (their
/// `crate::` is the test), test files and test items.
pub fn check(sources: &[Source], rules: &CrateRules) -> Vec<String> {
    let mut out = Vec::new();
    let files: Vec<(&syn::File, &str)> =
        sources.iter().filter(|s| !s.test).map(|s| (&s.file, s.module.as_str())).collect();
    let krate = Crate::new(&rules.deps, &files);
    for s in sources.iter().filter(|s| !s.test) {
        let (rel, module, names) = (&s.rel, &s.module, krate.named(&s.file, &s.module));
        for f in rules.forbid.iter().filter(|f| within(module, &f.from)) {
            if let Some(n) = names.iter().find(|n| within(n, &f.to)) {
                out.push(format!("{} names {} (forbid {} -> {})", rel, rules.show(n), f.from, f.to));
            }
        }
        rules.layer(rel, module, &names, &mut out);
        rules.confine(s, &names, &mut out);
    }
    out
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

    #[test]
    fn within_is_segment_wise() {
        assert!(within("board::dho::hw", "board") && within("board", "board") && !within("boarding", "board"));
    }

    fn source(rel: &str, module: &str, src: &str, test: bool) -> Source {
        Source { rel: rel.into(), module: module.into(), test, text: src.into(), file: parse(src) }
    }

    #[test]
    fn forbid_and_the_most_specific_layer_decide() {
        let sources = [
            source(
                "ui/src/render.rs",
                "render",
                "use crate::model::M;\nfn f() { crate::wave::w(); crate::render::gl::x(); oos_dsp::a(); }\n",
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
        let rules = CrateRules { forbid: vec![&f], layers: vec![&outer, &inner], confines: vec![], deps: deps() };
        let v = check(&sources, &rules);
        assert_eq!(v.len(), 4, "{:#?}", v);
        assert!(v[0].starts_with("ui/src/render.rs names crate::wave::w (forbid render -> wave)"));
        // `super::super::wave` from render::sub is crate::wave
        assert!(v[1].starts_with("ui/src/render/sub.rs names crate::wave::w (forbid render -> wave)"));
        assert!(v[2].starts_with("ui/src/render/sub.rs names crate::input::k (layer render "));
        assert!(v[3].starts_with("ui/src/render/gl/mod.rs names crate::model::m (layer render::gl"));
    }

    // The board boundary as rules: oos_hal::dho only in board::dho, board::dho only from itself.
    #[test]
    fn confined_paths_are_named_only_from_their_modules() {
        let sources = [
            source("ui/src/board/dho/hw.rs", "board::dho::hw", "use oos_hal::dho::seq;\n", false),
            source("ui/src/board/mod.rs", "board", "fn f() { dho::x(); crate::board::dho::y(); }\n", false),
            source("ui/src/ui.rs", "ui", "use oos_hal::dho::afg;\nuse oos_hal::model;\n", false),
            source("ui/src/ui/tests.rs", "ui::tests", "use oos_hal::dho::afg;\n", true),
        ];
        let hal = Confine { krate: "ui".into(), path: "oos_hal::dho".into(), allowed: vec!["board::dho".into()] };
        let dho = Confine { krate: "ui".into(), path: "board::dho".into(), allowed: vec![] };
        let rules = CrateRules { confines: vec![&hal, &dho], deps: deps(), ..CrateRules::default() };
        let v = check(&sources, &rules);
        assert_eq!(v.len(), 2, "{:#?}", v);
        assert!(v[0].starts_with("ui/src/board/mod.rs names crate::board::dho::y (confine board::dho"));
        assert!(v[1].starts_with("ui/src/ui.rs names oos_hal::dho::afg (confine oos_hal::dho"));
    }

    // A renamed dependency fails its confinement under the package name.
    #[test]
    fn a_renamed_dependency_is_confined_under_its_package_name() {
        let renamed: BTreeMap<String, String> = [("hal".to_string(), "oos_hal".to_string())].into();
        let src = "use hal::dho::seq;\n";
        let hal = Confine { krate: "ui".into(), path: "oos_hal::dho".into(), allowed: vec!["board::dho".into()] };
        let rules = CrateRules { confines: vec![&hal], deps: renamed, ..CrateRules::default() };
        let v = check(&[source("ui/src/ui.rs", "ui", src, false)], &rules);
        assert_eq!(v.len(), 1, "{:#?}", v);
        assert!(v[0].starts_with("ui/src/ui.rs names oos_hal::dho::"), "{}", v[0]);
    }

    // The hole: a macro body hid `crate::board::dho::Dho` from name resolution; the deleted text test saw it.
    #[test]
    fn a_confined_path_spelled_in_a_macro_fails() {
        let mac = "macro_rules! board_ty { () => { crate::board::dho::Dho } }\nfn stage(_: board_ty!()) {}\n";
        let hal_mac = "macro_rules! h { () => { hal::dho::seq::go() } }\n";
        let sources = [
            source("ui/src/stage.rs", "stage", mac, false),
            source("ui/src/other.rs", "other", hal_mac, false),
            source("ui/src/board/mod.rs", "board", mac, false),
            source("ui/src/board/sim.rs", "board::sim", "fn f() { m!(dho::Dho); m!(board::dho::Dho); }\n", false),
        ];
        let mut deps = deps();
        deps.insert("hal".into(), "oos_hal".into());
        let hal = Confine { krate: "ui".into(), path: "oos_hal::dho".into(), allowed: vec!["board::dho".into()] };
        let dho = Confine { krate: "ui".into(), path: "board::dho".into(), allowed: vec!["board".into()] };
        let rules = CrateRules { confines: vec![&hal, &dho], deps, ..CrateRules::default() };
        let v = check(&sources, &rules);
        assert_eq!(v.len(), 2, "{:#?}", v);
        assert!(v[0].starts_with("ui/src/stage.rs spells crate::board::dho::Dho in its tokens"), "{}", v[0]);
        assert!(v[1].starts_with("ui/src/other.rs spells hal::dho::seq::go in its tokens"), "{}", v[1]);
    }
}
