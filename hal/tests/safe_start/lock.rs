// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn wait(child: &mut std::process::Child) -> std::process::ExitStatus {
    let mut result = None;
    crate::support::wait_for("ledger writer completed", || {
        result = child.try_wait().unwrap();
        result.is_some()
    });
    result.unwrap()
}

#[test]
fn claim_publishes_the_owner_starttime_atomically() {
    let f = Fake::new("claim-identity");
    f.exec(
        "bin/sync",
        r#"#!/bin/sh
for claim in "$OOS_UI_TMP/oos-ui-exits.lock"/*; do
    printf '%s\n' "${claim##*/}" >> "$OOS_UI_TMP/claims"
done
exec "$OOS_REAL_SYNC" "$@"
"#,
    );
    let path = format!("{}:{}", f.p("bin").display(), std::env::var("PATH").unwrap());
    let mut writer = f
        .command("oos-ui-exits")
        .args(["count", "fresh", "1", "fresh"])
        .env("PATH", path)
        .env("OOS_REAL_SYNC", f.p("host-bin/sync"))
        .spawn()
        .unwrap();
    let expected = format!("{}.12345", writer.id());
    assert!(wait(&mut writer).success());
    let claims = std::fs::read_to_string(f.p("tmp/claims")).unwrap();
    assert!(claims.lines().all(|claim| claim == expected), "{}", claims);
    assert_eq!(claims.lines().count(), 2);
}

#[test]
fn reused_live_pid_does_not_keep_a_stale_claim() {
    let f = Fake::new("reused-pid");
    let pid = std::process::id();
    std::fs::create_dir_all(f.p(&format!("proc/{}", pid))).unwrap();
    // Linux field 22 follows a comm that itself contains spaces and closing parentheses.
    f.write(&format!("proc/{}/stat", pid), &format!("{} (old ) name) S {} 22222 0\n", pid, "0 ".repeat(18)));
    let stale = f.p(&format!("tmp/oos-ui-exits.lock/{}.11111", pid));
    std::fs::create_dir_all(&stale).unwrap();
    let mut writer = f.command("oos-ui-exits").args(["count", "fresh", "1", "fresh"]).spawn().unwrap();
    assert!(wait(&mut writer).success());
    assert!(!stale.exists());
    assert!(std::fs::read_to_string(f.p("oos/oos-ui.exits")).unwrap().contains("last fresh"));
}

#[test]
fn live_matching_identity_is_protected_until_released() {
    let f = Fake::new("live-identity");
    let pid = std::process::id();
    std::fs::create_dir_all(f.p(&format!("proc/{}", pid))).unwrap();
    f.write(&format!("proc/{}/stat", pid), &format!("{} (live) S {} 22222 0\n", pid, "0 ".repeat(18)));
    let claim = f.p(&format!("tmp/oos-ui-exits.lock/{}.22222", pid));
    std::fs::create_dir_all(&claim).unwrap();
    f.exec(
        "host-bin/sleep",
        r#"#!/bin/sh
touch "$OOS_UI_TMP/lock-wait"
"#,
    );
    let mut writer = f.command("oos-ui-exits").args(["count", "fresh", "1", "fresh"]).spawn().unwrap();
    crate::support::wait_for("writer observed live lock owner", || f.p("tmp/lock-wait").exists());
    let protected = writer.try_wait().unwrap().is_none() && claim.exists();
    std::fs::remove_dir(&claim).unwrap();
    assert!(wait(&mut writer).success());
    assert!(protected, "writer reaped a live matching process identity");
}
