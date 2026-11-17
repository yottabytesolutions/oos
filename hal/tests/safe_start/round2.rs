// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn path(f: &Fake) -> String {
    format!("{}:{}:{}", f.p("bin").display(), f.p("host-bin").display(), std::env::var("PATH").unwrap())
}
fn archives(f: &Fake) -> Vec<PathBuf> {
    std::fs::read_dir(f.p("oos"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("oos-ui.exits.bad."))
        .collect()
}

#[test]
fn read_only_timeout_preserves_healthy_history() {
    timeout_preserves_history("mode");
}

#[test]
fn killed_archive_preserves_healthy_history() {
    timeout_preserves_history("crash");
}

fn timeout_preserves_history(op: &str) {
    let f = Fake::new(&format!("round2-{}", op));
    f.count("old", "1");
    let before = std::fs::read(f.p("oos/oos-ui.exits")).unwrap();
    f.exec(
        "bin/timeout",
        r#"#!/bin/sh
[ "$1" != -s ] || shift 2
bound=$1; shift
if [ ! -e "$OOS_UI_TMP/failed" ]; then
 case "$OOS_FAIL_OP:$bound:$2" in mode:5:mode|crash:20:*) touch "$OOS_UI_TMP/failed"; exit 124 ;; esac
fi
exec "$@"
"#,
    );
    f.exec(
        "bin/ui",
        r#"#!/bin/sh
echo "$*" >> "$OOS_UI_TMP/launches"
[ "$(wc -l < "$OOS_UI_TMP/launches")" -ne 2 ] || touch "$OOS_UI_TMP/oos-ui.off"
"#,
    );
    let out = f.command("oos-ui-loop").env("PATH", path(&f)).env("OOS_FAIL_OP", op).output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    let launches = std::fs::read_to_string(f.p("tmp/launches")).unwrap();
    assert!(launches.lines().next().unwrap().contains("--safe"));
    assert!(!launches.lines().nth(1).unwrap().contains("--safe"));
    assert_eq!(std::fs::read(f.p("oos/oos-ui.exits")).unwrap(), before, "{}", op);
    assert!(archives(&f).is_empty());
}

#[test]
fn recovery_leaves_a_healthy_ledger_untouched() {
    let f = Fake::new("round2-prune");
    f.count("healthy", "1");
    let before = std::fs::read(f.p("oos/oos-ui.exits")).unwrap();
    assert!(f.command("oos-ui-exits").arg("recover").output().unwrap().status.success());
    assert_eq!(std::fs::read(f.p("oos/oos-ui.exits")).unwrap(), before);
}

#[test]
fn damaged_archive_pruning_keeps_two_newest_across_digit_boundary() {
    let f = Fake::new("round2-archive-prune");
    for stamp in [8, 9, 10, 11] {
        f.write("now", &stamp.to_string());
        f.write("oos/oos-ui.exits", &format!("broken {}\n", stamp));
        assert!(f.command("oos-ui-exits").arg("recover").output().unwrap().status.success());
        assert!(archives(&f).len() <= 2);
    }
    let mut kept: Vec<_> = archives(&f).iter().map(|p| std::fs::read_to_string(p).unwrap()).collect();
    kept.sort();
    assert_eq!(kept, ["broken 10\n", "broken 11\n"]);
}

#[test]
fn concurrent_writers_wait_for_commit_and_reclaim_killed_writer_lock() {
    let f = Fake::new("round2-lock");
    f.count("old", "1");
    f.exec(
        "bin/sync",
        r#"#!/bin/sh
if [ "$1" = "$OOS_UI_DATA/oos-ui.exits.new" ] && [ ! -e "$OOS_UI_TMP/entered" ]; then
 touch "$OOS_UI_TMP/entered"
 while [ ! -e "$OOS_UI_TMP/release" ]; do sleep 0.01; done
fi
exec "$OOS_REAL_SYNC" "$@"
"#,
    );
    let mut first = f
        .command("oos-ui-exits")
        .args(["count", "first", "1", "first"])
        .env("PATH", path(&f))
        .env("OOS_REAL_SYNC", f.p("host-bin/sync"))
        .spawn()
        .unwrap();
    crate::support::wait_for("first ledger writer reached commit", || f.p("tmp/entered").exists());
    f.exec(
        "host-bin/sleep",
        r#"#!/bin/sh
if [ "$1" = 1 ]; then touch "$OOS_UI_TMP/lock-wait"; fi
"#,
    );
    let mut second = f.command("oos-ui-exits").args(["count", "second", "1", "second"]).spawn().unwrap();
    crate::support::wait_for("second ledger writer observed first writer's lock", || f.p("tmp/lock-wait").exists());
    let raced = second.try_wait().unwrap().is_some();
    f.write("tmp/release", "");
    assert!(first.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
    assert!(!raced, "second writer committed while first still owned the temporary file");
    assert_eq!(f.mode(), "safe");
    // A dead writer cannot retain the lock forever, including a kill before pid publication.
    for owner in [Some("999999999"), None] {
        let lock = f.p("tmp/oos-ui-exits.lock");
        std::fs::create_dir(&lock).unwrap();
        if let Some(owner) = owner {
            std::fs::create_dir(lock.join(owner)).unwrap();
        }
        f.count("after-kill", "1");
        assert!(!lock.exists());
    }
}
