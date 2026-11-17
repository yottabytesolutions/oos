// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn fixture(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("xtask-plan-matrix-{name}-{}", std::process::id()));
    std::fs::create_dir_all(root.join("xtask/data")).unwrap();
    std::fs::create_dir_all(root.join("tools")).unwrap();
    std::fs::write(
        root.join("tools/output.py"),
        "# SPDX-License-Identifier: GPL-2.0-or-later\nfrom pathlib import Path\nprint(Path(__file__).resolve().parent.parent)\n",
    )
    .unwrap();
    root
}

#[test]
fn split_ci_gates_check_register_plans() {
    let pr = crate::stack_ci::parse(include_str!("../../../.github/workflows/pr.yml")).unwrap();
    assert!(pr["jobs"]["clippy-run"]["strategy"]["matrix"]["include"]
        .as_vec()
        .unwrap()
        .iter()
        .any(|job| { job["steps"].as_str().unwrap().split_whitespace().any(|id| id == "plan-matrix") }));
    assert!(crate::stack_ci::steps(&pr, "clippy-run").iter().any(|step| {
        step["env"]["STEPS"].as_str() == Some("${{ matrix.steps }}")
            && step["run"].as_str().is_some_and(|run| run.contains("cargo xtask step \"${ids[@]}\""))
    }));
    let nightly = crate::stack_ci::parse(include_str!("../../../.github/workflows/main.yml")).unwrap();
    assert!(nightly["jobs"]["gate"]["strategy"]["matrix"]["include"].as_vec().unwrap().iter().any(|job| {
        job["name"].as_str() == Some("clippy")
            && job["steps"].as_str().unwrap().split_whitespace().any(|id| id == "plan-matrix")
    }));
}

#[test]
fn shell_and_host_use_the_shared_cases_in_order() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture("shell matrix");
    let script = root.join("tools/plan-matrix.sh");
    std::fs::write(&script, include_str!("../../../tools/plan-matrix.sh")).unwrap();
    let binary = root.join("fake fpgad");
    std::fs::write(&binary, "#!/bin/sh\nprintf '<%s>' \"$@\"\nprintf '\\n'\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    let definition = include_str!("../../../tools/plan-matrix-cases.txt");
    let expected = generate(|args| {
        Ok(run::Output {
            ok: true,
            stdout: format!("{}\n", args.iter().map(|arg| format!("<{arg}>")).collect::<String>()),
            stderr: String::new(),
        })
    })
    .unwrap();
    let cases = cases();
    assert_eq!(cases.len(), 65);
    assert_eq!(cases.iter().collect::<std::collections::BTreeSet<_>>().len(), 65);
    for (definition, expected) in
        [(definition, expected.as_str()), ("--frames 16\n", "### --frames 16\n<plan><--frames><16>\n")]
    {
        std::fs::write(root.join("tools/plan-matrix-cases.txt"), definition).unwrap();
        let output = std::process::Command::new("sh").arg(&script).arg(&binary).current_dir(&root).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn preserves_output_and_rejects_failed_plans_even_when_they_print_pass() {
    let matrix = generate(|args| {
        assert_eq!(args[0], "plan");
        Ok(run::Output { ok: true, stdout: "  wr  0x2000 0x00000001\n".into(), stderr: "# diagnostic\n".into() })
    })
    .unwrap();
    assert!(
        matrix.starts_with("### --mask 1 --app-mask 1 --src 1 --tdiv 1e-9\n  wr  0x2000 0x00000001\n# diagnostic\n")
    );
    assert_eq!(matrix.matches("### ").count(), 65);
    let failed = generate(|_| Ok(run::Output { ok: false, stdout: "PASS\n".into(), stderr: "bad plan\n".into() }));
    let error = failed.unwrap_err();
    assert!(error.contains("--mask 1") && error.contains("PASS") && error.contains("bad plan"));
    assert_eq!(generate(|_| Err("could not start".into())).unwrap_err(), "could not start");
}

#[test]
fn checking_never_changes_the_baseline_and_update_is_explicit() {
    let root = fixture("compare");
    let ctx = Context::default();
    let before = "### case\n  wr  0x2000 0x00000001\n";
    let after = "### case\n  wr  0x2000 0x00000002\n";
    assert!(compare(&ctx, &root, before, true).is_ok());
    assert!(compare(&ctx, &root, before, false).is_ok());
    let failure = compare(&ctx, &root, after, false).unwrap_err().join("\n");
    assert!(failure.contains("--- xtask/data/plan-matrix.txt"));
    assert!(failure.contains("+++ generated plan matrix"));
    assert!(failure.contains("-  wr  0x2000 0x00000001") && failure.contains("+  wr  0x2000 0x00000002"));
    assert!(failure.contains("cargo xtask plan-matrix --update"));
    assert_eq!(std::fs::read_to_string(root.join(BASELINE)).unwrap(), before);
    assert!(compare(&ctx, &root, after, true).is_ok());
    assert!(compare(&ctx, &root, after, false).is_ok());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_baselines_fail_with_the_update_hint() {
    let root = fixture("missing");
    let failure = compare(&Context::default(), &root, "plan\n", false).unwrap_err().join("\n");
    assert!(failure.contains(BASELINE) && failure.contains("cargo xtask plan-matrix --update"));
    assert!(!root.join(BASELINE).exists());
    std::fs::remove_dir_all(root).unwrap();
}
