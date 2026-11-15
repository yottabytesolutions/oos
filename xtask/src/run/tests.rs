use super::*;
use std::{ffi::OsString, fs};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("oos-context-{}-{stamp}", std::process::id()));
        fs::create_dir_all(root.join("tools")).unwrap();
        // The selector is a fixture; real worktree selection remains tools/output.py's responsibility.
        fs::write(
            root.join("tools/output.py"),
            "from pathlib import Path\nprint(Path(__file__).resolve().parent.parent / 'selected target')\n",
        )
        .unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn start(&self) -> Context {
        let (ctx, logs) = start_logs(&self.0).unwrap();
        assert_eq!(ctx.target.as_ref().unwrap(), &self.0.join("selected target"));
        assert_eq!(ctx.logs.as_ref(), Some(&logs));
        assert!(logs.is_dir());
        ctx
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn environment() -> [Option<OsString>; 2] {
    [std::env::var_os("CARGO_TARGET_DIR"), std::env::var_os("OOS_CHECK_LOG_DIR")]
}

fn logs(ctx: &Context) -> Vec<String> {
    fs::read_dir(ctx.logs.as_ref().unwrap()).unwrap().map(|p| fs::read_to_string(p.unwrap().path()).unwrap()).collect()
}

const ECHO_ENV: &str = "printf '%s\n%s\n' \"$CARGO_TARGET_DIR\" \"$OOS_CHECK_LOG_DIR\"";

fn expected(ctx: &Context, target: &Path) -> String {
    format!("{}\n{}\n", target.display(), ctx.logs.as_ref().unwrap().display())
}

#[test]
fn tail_keeps_the_last_lines() {
    assert_eq!(tail("a\nb\nc", 2), "b\nc");
    assert_eq!(tail("a", 5), "a");
    assert_eq!(tail("", 3), "");
}

#[test]
fn starting_sessions_and_children_leave_the_inherited_environment_unchanged() {
    let before = environment();
    let fixture = Fixture::new();
    let a = fixture.start();
    let b = fixture.start();
    assert_ne!(a.logs, b.logs);
    assert_eq!(environment(), before);
    let child = capture(&a, &fixture.0, "sh", &["-c", ECHO_ENV]).unwrap();
    assert!(child.ok);
    assert_eq!(child.stdout, expected(&a, a.target.as_ref().unwrap()));
    assert_eq!(environment(), before);
    let inherited = capture(&Context::default(), &fixture.0, "sh", &["-c", ECHO_ENV]).unwrap();
    let inherited_text = before.map(|v| v.unwrap_or_default().to_string_lossy().into_owned());
    assert_eq!(inherited.stdout, format!("{}\n{}\n", inherited_text[0], inherited_text[1]));
    assert_eq!(logs(&a).len(), 1);
    assert!(logs(&b).is_empty());
}

#[test]
fn concurrent_inventory_steps_keep_their_own_targets_and_logs() {
    let before = environment();
    let a = Fixture::new();
    let b = Fixture::new();
    // Exercise the actual inventory runner, including its Python command capture.
    let script = "import os\nprint(os.environ['CARGO_TARGET_DIR'])\nprint(os.environ['OOS_CHECK_LOG_DIR'])\n";
    for fixture in [&a, &b] {
        fs::write(fixture.0.join("tools/check_inventory.py"), script).unwrap();
    }
    let contexts = [a.start(), b.start()];
    let barrier = std::sync::Barrier::new(2);
    let run = crate::inventory::CHECKS.iter().find(|c| c.id == "python-tests").unwrap().run;
    std::thread::scope(|scope| {
        for (fixture, ctx) in [&a, &b].into_iter().zip(&contexts) {
            let barrier = &barrier;
            scope.spawn(move || {
                barrier.wait();
                for _ in 0..4 {
                    let lines = run(ctx, &fixture.0).unwrap();
                    assert_eq!(lines, [expected(ctx, ctx.target.as_ref().unwrap())]);
                }
            });
        }
    });
    for ctx in &contexts {
        let retained = logs(ctx);
        assert_eq!(retained.len(), 4);
        assert!(retained.iter().all(|text| text.contains(&expected(ctx, ctx.target.as_ref().unwrap()))));
    }
    assert_eq!(environment(), before);
}

#[test]
fn explicit_target_override_keeps_the_session_logs_and_does_not_leak() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    let before = environment();
    let coverage = ctx.target.as_ref().unwrap().join("xtask-cov");
    let out =
        capture_env(&ctx, &fixture.0, "sh", &["-c", ECHO_ENV], &[("CARGO_TARGET_DIR", coverage.as_os_str())]).unwrap();
    assert_eq!(out.stdout, expected(&ctx, &coverage));
    assert_eq!(environment(), before);
    let out = capture(&ctx, &fixture.0, "sh", &["-c", ECHO_ENV]).unwrap();
    assert_eq!(out.stdout, expected(&ctx, ctx.target.as_ref().unwrap()));
    assert_eq!(logs(&ctx).len(), 2);
    assert!(logs(&ctx).iter().any(|text| text.contains(&expected(&ctx, &coverage))));
}

#[test]
fn failed_commands_retain_complete_output_and_start_errors() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    let script = "echo PASS; i=0; while [ $i -lt 100 ]; do echo out-$i; echo err-$i >&2; i=$((i+1)); done; exit 7";
    let out = capture(&ctx, &fixture.0, "sh", &["-c", script]).unwrap();
    assert!(!out.ok);
    assert_eq!(out.stdout.lines().count(), 101);
    assert_eq!(out.stderr.lines().count(), 100);
    let retained = logs(&ctx);
    assert_eq!(retained.len(), 1);
    assert!(retained[0].contains(&format!("stdout:\n{}\nstderr:\n{}", out.stdout, out.stderr)));
    let error = capture(&ctx, &fixture.0, "oos-nonexistent-fixture-command", &[]).unwrap_err();
    assert!(error.contains("oos-nonexistent-fixture-command"));
    assert_eq!(logs(&ctx).len(), 2);
    assert!(logs(&ctx).iter().any(|text| text.contains("failed to start:")));
    assert!(crate::steps::host::command(&ctx, &fixture.0, "sh", &["-c", "echo PASS; exit 7"]).is_err());
}

#[test]
fn selector_failure_does_not_create_a_session_or_change_the_environment() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("tools/output.py"), "import sys\nprint('selector failed', file=sys.stderr)\nsys.exit(7)")
        .unwrap();
    let before = environment();
    assert!(start_logs(&fixture.0).unwrap_err().contains("selector failed"));
    assert!(!fixture.0.join("selected target").exists());
    assert_eq!(environment(), before);
}

#[test]
fn formatting_metadata_and_git_helpers_retain_logs_in_the_explicit_session() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    let root = crate::repo::root();
    assert_eq!(crate::fmt::diff_lines(&ctx, &root, "xtask").unwrap(), 0);
    assert!(!crate::workspace::load(&ctx, &root).unwrap().packages.is_empty());
    assert!(crate::base::protected(&ctx, &root).is_ok());
    let retained = logs(&ctx);
    assert!(retained.iter().any(|text| text.starts_with("cargo [\"fmt\"")));
    assert!(retained.iter().any(|text| {
        text.starts_with("cargo [\"metadata\"") && text.contains(ctx.target.as_ref().unwrap().to_str().unwrap())
    }));
    assert!(retained.iter().any(|text| text.starts_with("git [\"merge-base\"")));
}

#[test]
fn byte_capture_preserves_invalid_utf8_for_inventory_validation() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    let out = capture_raw(&ctx, &fixture.0, "sh", &["-c", "printf '\\377\\000'"], &[]).unwrap();
    assert_eq!(out.stdout, [0xff, 0]);
    assert_eq!(logs(&ctx).len(), 1);
}

#[test]
fn shell_and_image_inventory_runners_forward_the_context() {
    let fixture = Fixture::new();
    let ctx = fixture.start();
    let python = "import os\nprint(os.environ['CARGO_TARGET_DIR'])\nprint(os.environ['OOS_CHECK_LOG_DIR'])\n";
    for file in ["script_syntax.py", "check_inventory.py", "image_required.py", "image_suite.py"] {
        fs::write(fixture.0.join("tools").join(file), python).unwrap();
    }
    fs::create_dir(fixture.0.join("gpu")).unwrap();
    fs::write(fixture.0.join("gpu/test-host.sh"), ECHO_ENV).unwrap();
    let ids =
        ["script-syntax", "arm64-release-link", "ssh-sim", "firewall-sim", "gpu-host", "image-required", "image-suite"];
    for id in ids {
        let step = crate::inventory::CHECKS.iter().find(|c| c.id == id).unwrap();
        assert_eq!((step.run)(&ctx, &fixture.0).unwrap(), [expected(&ctx, ctx.target.as_ref().unwrap())]);
    }
    assert_eq!(logs(&ctx).len(), ids.len());
}
