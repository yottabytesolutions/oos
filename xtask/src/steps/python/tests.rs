// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::steps::host::tests::Fixture;

#[test]
fn shared_python_runner_is_isolated_and_propagates_failure_without_building() {
    let f = Fixture::new("python-run");
    f.output("build");
    for status in [0, 7] {
        f.write(
            "tools/python_tests.py",
            &format!("import sys\nassert sys.flags.isolated\nprint('runner output')\nsys.exit({status})\n"),
        );
        let result = run(&Default::default(), &f.0);
        assert_eq!(result.is_ok(), status == 0);
        assert_eq!(result.unwrap_or_else(|lines| lines), ["runner output\n"]);
        assert!(!f.0.join("commands").exists(), "Python tests must not build or verify Rust binaries");
    }
}
