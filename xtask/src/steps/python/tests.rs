// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::steps::host::tests::Fixture;

#[test]
fn runner_builds_the_host_updater_in_the_selected_target_and_propagates_failure() {
    let f = Fixture::new("python-run");
    f.output("");
    for status in [0, 7] {
        f.write(
            "tools/python_tests.py",
            &format!(
                "import os, sys\nfrom pathlib import Path\nassert sys.flags.isolated\n\
                 updater = Path.cwd() / 'selected target/debug/oos-update'\n\
                 assert Path(os.environ['OOS_RELEASE_TEST_UPDATER']) == updater\n\
                 print('runner output')\nsys.exit({status})\n"
            ),
        );
        let result = run(&Context::default(), &f.0);
        assert_eq!(result.is_ok(), status == 0);
        assert_eq!(result.unwrap_or_else(|lines| lines).last().unwrap(), "runner output\n");
    }
    let commands = std::fs::read_to_string(f.0.join("commands")).unwrap();
    assert_eq!(commands, "build -p oos-fpgad --bin oos-update\ntarget\n".repeat(2));
    f.output("build");
    assert!(run(&Context::default(), &f.0).unwrap_err().join("\n").contains("failed build"));
    assert_eq!(
        std::fs::read_to_string(f.0.join("commands")).unwrap(),
        commands + "build -p oos-fpgad --bin oos-update\n"
    );
}
