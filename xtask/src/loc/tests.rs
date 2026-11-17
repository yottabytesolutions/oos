// SPDX-License-Identifier: GPL-2.0-or-later
use super::report::{Report, Row};
use super::*;
use crate::testrepo;

#[test]
fn flags_parse_and_refuse_unknown_or_repeated_ones() {
    let o = Options::parse(&["--json", "--compare", "v0.2.0"]).expect("flags");
    assert_eq!(o, Options { json: true, compare: Some("v0.2.0".into()) });
    for bad in [&["--compare"][..], &["--compare", "--json"], &["--json", "--json"], &["--fast"]] {
        assert!(Options::parse(bad).is_err(), "{:?}", bad);
    }
}

#[test]
fn scripts_are_known_by_extension_or_shebang_and_tests_are_left_out() {
    assert_eq!(tooling::kind("a.py", ""), Some("python"));
    assert_eq!(tooling::kind("a.sh", ""), Some("shell"));
    assert_eq!(tooling::kind("oos-ui-loop", "#!/bin/busybox sh"), Some("shell"));
    assert_eq!(tooling::kind("scope-lease", "#!/usr/bin/env python3"), Some("python"));
    assert_eq!(tooling::kind("README.md", "#!/bin/sh"), None);
    assert_eq!(tooling::kind("LICENSE", "GNU"), None);
    assert!(tooling::is_test("tools/tests/helper.py") && tooling::is_test("tools/test_x.py"));
    assert!(tooling::is_test("ui/tests/golden/a.sh") && !tooling::is_test("tools/ci/pr_meta.py"));
    assert_eq!(tooling::lines("#!/bin/sh\n# note\n\nset -eu\n  echo x # trailing\n"), 2);
}

#[test]
fn c_lines_leave_out_comments_but_not_literals() {
    let src = "// SPDX\n#include <x.h>\n\n/* block\n   still */ int a;\nconst char *s = \"/* no */\"; // tail\nchar q = '\\'';\n/** doc */\n";
    assert_eq!(c::lines(src), 4);
}

#[test]
fn an_extracted_tree_holds_sources_manifests_and_scripts_but_no_toolchain_pin() {
    for kept in ["gpu/oos-gpu.c", "ui/src/lib.rs", "Cargo.toml", "Cargo.lock", "tools/a.py", "boot/b.sh", "rootfs/init"]
    {
        assert!(tree::needed(kept), "{}", kept);
    }
    for dropped in ["rust-toolchain.toml", "ui/tests/golden/a.png", "docs/x.md"] {
        assert!(!tree::needed(dropped), "{}", dropped);
    }
}

fn counts(production: &[(&str, u64)], tooling: &[(&str, u64)]) -> Counts {
    let m = |v: &[(&str, u64)]| v.iter().map(|(k, n)| (k.to_string(), *n)).collect();
    Counts { production: m(production), tooling: m(tooling) }
}

#[test]
fn the_report_compares_every_name_on_either_side() {
    let now = counts(&[("core", 90), ("ui", 200)], &[("shell", 5)]);
    let base = counts(&[("core", 100), ("old", 10)], &[("shell", 5)]);
    let r = Report::new(&now, Some(("v0.2.0", &base)));
    let row = |now, base, delta| Row { now, base: Some(base), delta: Some(delta) };
    assert_eq!(r.production["old"], row(0, 10, -10));
    assert_eq!(r.production["ui"], row(200, 0, 200));
    assert_eq!(r.production_total, row(290, 110, 180));
    let md = r.markdown();
    assert!(md.starts_with("| Production code | v0.2.0 | Now | Change |\n|---|---:|---:|---:|\n"), "{}", md);
    assert!(md.contains("| core | 100 | 90 | -10 |\n") && md.contains("| Total | 110 | 290 | +180 |\n"), "{}", md);
    assert!(md.contains("| shell | 5 | 5 | +0 |\n"), "{}", md);
    let json = serde_json::to_value(&r).expect("json");
    assert_eq!(json["production_total"]["delta"], 180);
    assert_eq!(json["base"], "v0.2.0");
}

#[test]
fn without_a_comparison_the_report_has_one_column() {
    let r = Report::new(&counts(&[("core", 7)], &[]), None);
    assert_eq!(
        r.markdown(),
        "| Production code | Lines |\n|---|---:|\n| core | 7 |\n| Total | 7 |\n\n| Tooling | Lines |\n|---|---:|\n"
    );
    assert_eq!(serde_json::to_string(&r.production_total).expect("json"), r#"{"now":7}"#);
}

const LIB: &str = "//! doc\n/// item\npub fn f() -> u8 {\n    // note\n\n    1\n}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";

fn workspace(lib: &str, script: &str) -> Vec<(&'static str, String)> {
    let manifest = |n: &str| format!("[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n", n);
    vec![
        ("Cargo.toml", "[workspace]\nmembers = [\"core\", \"xtask\"]\nresolver = \"2\"\n".into()),
        ("core/Cargo.toml", manifest("p-core")),
        ("core/src/lib.rs", lib.into()),
        ("core/src/gen.rs", "// @generated\npub fn g() {}\n".into()),
        ("core/tests/it.rs", "#[test]\nfn i() {}\n".into()),
        ("xtask/Cargo.toml", manifest("xtask")),
        ("xtask/src/main.rs", "fn main() {}\n".into()),
        ("tools/run", script.into()),
        ("tools/tests/test_run.py", "x = 1\n".into()),
        ("gpu/g.c", "// note\nint g;\n".into()),
        ("kernel/modules/xdma/v.c", "int v;\n".into()),
    ]
}

fn files<'a>(v: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    v.iter().map(|(p, t)| (*p, t.as_str())).collect()
}

#[test]
fn a_ref_is_measured_like_the_working_tree() {
    let first = workspace(&format!("{}mod gen;\n", LIB), "#!/bin/sh\n# note\necho a\n");
    let second =
        workspace(&format!("{}mod gen;\npub fn h() {{}}\n", LIB), "#!/usr/bin/env python3\nprint(1)\nprint(2)\n");
    let dir = testrepo::repo("loc", &[(&files(&first), "first"), (&files(&second), "second")]);
    let now = measure(&dir, &tree::files(&dir, None).expect("files"));
    let then = tree::at(&dir, "HEAD~1", measure);
    std::fs::remove_dir_all(&dir).expect("clean up");
    let (now, then) = (now.expect("now"), then.expect("then"));
    assert_eq!(then, counts(&[("core", 4), ("gpu", 1)], &[("python", 0), ("shell", 1), ("xtask", 1)]));
    assert_eq!(now, counts(&[("core", 5), ("gpu", 1)], &[("python", 2), ("shell", 0), ("xtask", 1)]));
}
