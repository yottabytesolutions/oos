// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::steps::host::tests::Fixture;

#[test]
fn summary_requires_exact_nonzero_counts_and_every_skip_reason() {
    assert_eq!(summary("Ran 1 test in 0.1s\nOK", 1).unwrap(), ["python-tests: 1 executed; 0 skipped"]);
    let skipped = "test_fixture (fixture.Tests) ... skipped 'requires fixture'\nRan 2 tests in 0.1s\nOK (skipped=1)";
    assert_eq!(
        summary(skipped, 2).unwrap(),
        ["python-tests: 2 executed; 1 skipped", "python-tests: skip reason: 'requires fixture'",]
    );
    for (text, expected) in [
        ("Ran 0 tests in 0s", 0),
        ("Ran 0 tests in 0s", 2),
        ("PASS", 2),
        ("Ran 1 test in 0s", 2),
        ("Ran 2 tests in 0s\nRan 2 tests in 0s", 2),
        ("Ran 2 tests in 0s\nOK (skipped=1)", 2),
        ("a ... skipped 'why'\nb ... skipped 'why'\nRan 2 tests in 0s\nOK (skipped=2)", 2),
    ] {
        assert!(summary(text, expected).is_err(), "{text}");
    }
}

#[test]
fn discovery_rejects_unregistered_tests_and_ignores_deleted_files() {
    let f = Fixture::new("python-roots");
    assert!(validate_roots(&Default::default(), &f.0).is_err());
    for directory in ROOTS {
        f.write(&format!("{directory}/test_fixture.py"), "pass\n");
    }
    assert!(validate_roots(&Default::default(), &f.0).is_ok());
    for name in ["test_new.py", "nested/test_new.py", "tools/test_new.py", "tools/tests-other/test_new.py"] {
        f.write(name, "pass\n");
        assert!(validate_roots(&Default::default(), &f.0).unwrap_err().contains(name));
        std::fs::remove_file(f.0.join(name)).unwrap();
    }
    crate::testrepo::git(&f.0, &["add", "."]);
    std::fs::remove_file(f.0.join("ui/tools/test_fixture.py")).unwrap();
    assert!(validate_roots(&Default::default(), &f.0).is_ok());
}

const PASS: &str = "import unittest\nclass Tests(unittest.TestCase):\n    def test_pass(self): pass\n";

#[test]
fn isolated_roots_with_matching_module_names_run_after_a_verified_build() {
    let f = Fixture::new("python-run");
    f.output("");
    for directory in ROOTS {
        f.write(&format!("{directory}/test_fixture.py"), PASS);
    }
    let lines = run(&Default::default(), &f.0).unwrap();
    assert_eq!(lines.iter().filter(|l| *l == "python-tests: 1 executed; 0 skipped").count(), ROOTS.len());
    let commands = std::fs::read_to_string(f.0.join("commands")).unwrap();
    assert!(commands.starts_with("build -p oos-ui\ntarget\nverify "));
    assert!(commands.contains("selected target/debug/oos-ui"));
    f.output("verify");
    assert!(run(&Default::default(), &f.0).is_err());
}

#[test]
fn rejects_empty_failed_discovery_failed_tests_and_all_skipped_runs() {
    let f = Fixture::new("python-failures");
    f.write("tests/empty.py", "");
    assert!(tests(&Default::default(), &f.0, "tests").unwrap_err().join("\n").contains("zero tests"));
    for text in [
        "raise RuntimeError('broken import')\n",
        "import unittest\nclass Tests(unittest.TestCase):\n    def test_fail(self): self.fail('FAIL')\n",
        "import unittest\n\
         @unittest.skip('fixture unavailable')\nclass Tests(unittest.TestCase):\n    def test_skip(self): pass\n",
    ] {
        f.write("tests/test_fixture.py", text);
        assert!(tests(&Default::default(), &f.0, "tests").is_err(), "{text}");
    }
}
