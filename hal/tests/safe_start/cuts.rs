// SPDX-License-Identifier: GPL-2.0-or-later
//! Kill the actual ledger writer on both sides of each external durable operation.
use super::Fake;

fn real(command: &str) -> String {
    let output = std::process::Command::new("sh").args(["-c", &format!("command -v {}", command)]).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn interrupt(f: &Fake, command: &str, call: u32, after: bool) {
    let real = real(command);
    let cut = if after {
        format!("'{real}' \"$@\" || exit 1\nkill -KILL \"$PPID\"\nexit 1")
    } else {
        "kill -KILL \"$PPID\"\nexit 1".into()
    };
    f.exec(
        &format!("bin/{}", command),
        &format!(
            r#"#!/bin/sh
n=$(cat "$OOS_UI_TMP/calls" 2>/dev/null || echo 0)
n=$((n + 1)); echo "$n" > "$OOS_UI_TMP/calls"
if [ "$n" -eq {call} ]; then
{cut}
fi
exec '{real}' "$@"
"#
        ),
    );
    let path = format!("{}:{}", f.p("bin").display(), std::env::var("PATH").unwrap());
    let output = f.command("oos-ui-exits").args(["count", "new", "1", "new"]).env("PATH", path).output().unwrap();
    assert!(!output.status.success(), "writer survived {} {} {}", command, call, after);
}

#[test]
fn ledger_kills_at_every_commit_boundary_count_exactly_once_after_retry() {
    for (command, call) in [("sync", 1), ("mv", 1), ("sync", 2)] {
        for after in [false, true] {
            for lose_namespace in [false, true] {
                let f = Fake::new("ledger-cuts");
                f.count("old", "1");
                let before = std::fs::read(f.p("oos/oos-ui.exits")).unwrap();
                interrupt(&f, command, call, after);
                // Before the directory fsync, either namespace may survive. Restoring
                // the previous durable ledger models loss of the uncommitted rename.
                if lose_namespace && !(command == "sync" && call == 2 && after) {
                    std::fs::write(f.p("oos/oos-ui.exits"), before).unwrap();
                }
                f.count("new", "1");
                f.count("new", "1");
                let ledger = std::fs::read_to_string(f.p("oos/oos-ui.exits")).unwrap();
                assert_eq!(ledger.lines().filter(|s| s.starts_with("10000 ")).count(), 2);
                assert_eq!(f.mode(), "normal");
                f.count("third", "1");
                assert_eq!(f.mode(), "safe");
            }
        }
    }
}

#[test]
fn interrupted_temporary_ledger_never_replaces_the_previous_one() {
    let f = Fake::new("ledger-partial");
    f.count("old", "1");
    f.exec("bin/awk", "#!/bin/sh\nprintf 'last partial'\nkill -KILL \"$PPID\"\nexit 1\n");
    let path = format!("{}:{}", f.p("bin").display(), std::env::var("PATH").unwrap());
    let output = f.command("oos-ui-exits").args(["count", "new", "1", "new"]).env("PATH", path).output().unwrap();
    assert!(!output.status.success());
    f.count("new", "1");
    assert_eq!(f.mode(), "normal");
    f.count("third", "1");
    assert_eq!(f.mode(), "safe");
}
