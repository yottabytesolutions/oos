// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

fn clean_ui(f: &Fake) {
    f.exec(
        "bin/ui",
        r#"#!/bin/sh
echo "$*" >> "$OOS_UI_TMP/launches"
touch "$OOS_UI_TMP/oos-ui.off"
"#,
    );
}
fn ledger(f: &Fake) -> String {
    std::fs::read_to_string(f.p("oos/oos-ui.exits")).unwrap_or_default()
}
fn path(f: &Fake) -> String {
    format!("{}:{}:{}", f.p("bin").display(), f.p("host-bin").display(), std::env::var("PATH").unwrap())
}

#[test]
fn mode_never_writes_or_syncs_the_ledger() {
    let f = Fake::new("read-only-mode");
    f.count("old", "1");
    let before = ledger(&f);
    let modified = std::fs::metadata(f.p("oos/oos-ui.exits")).unwrap().modified().unwrap();
    f.exec("bin/sync", "#!/bin/sh\necho sync >> \"$OOS_UI_TMP/syncs\"\nexit 1\n");
    let out = f.command("oos-ui-exits").arg("mode").env("PATH", path(&f)).output().unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(ledger(&f), before);
    assert_eq!(std::fs::metadata(f.p("oos/oos-ui.exits")).unwrap().modified().unwrap(), modified);
    assert!(!f.p("tmp/syncs").exists());
}

#[test]
fn damaged_ledger_is_kept_and_selects_safe_once() {
    let f = Fake::new("damaged-ledger");
    f.write("oos/oos-ui.exits", "damaged\n");
    clean_ui(&f);
    assert!(f.run().status.success());
    assert!(std::fs::read_to_string(f.p("tmp/launches")).unwrap().contains("--safe"));
    let kept: Vec<_> = std::fs::read_dir(f.p("oos"))
        .unwrap()
        .flatten()
        .filter(|p| p.file_name().to_string_lossy().starts_with("oos-ui.exits.bad."))
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(std::fs::read_to_string(kept[0].path()).unwrap(), "damaged\n");
    std::fs::remove_file(f.p("tmp/oos-ui.off")).unwrap();
    assert!(f.run().status.success());
    assert!(!std::fs::read_to_string(f.p("tmp/launches")).unwrap().lines().nth(1).unwrap().contains("--safe"));
}

#[test]
fn clear_does_not_depend_on_awk_or_old_ledger_parsing() {
    let f = Fake::new("clear-awk");
    f.write("oos/oos-ui.exits", "damaged\n");
    f.exec("bin/awk", "#!/bin/sh\nexit 1\n");
    assert!(f.command("oos-ui-exits").arg("clear").env("PATH", path(&f)).output().unwrap().status.success());
    assert_eq!(f.mode(), "normal");
}

#[test]
fn early_crashes_use_fresh_launch_ids_despite_an_old_marker() {
    let f = Fake::new("early-crash");
    f.write("oos/oos-ui.stable", "previous-boot:old\n");
    f.exec(
        "bin/ui",
        r#"#!/bin/sh
echo "$*" >> "$OOS_UI_TMP/launches"
echo "$OOS_UI_RUN_ID" >> "$OOS_UI_TMP/ids"
n=$(wc -l < "$OOS_UI_TMP/launches")
if [ "$n" -eq 4 ]; then touch "$OOS_UI_TMP/oos-ui.off"; exit 0; fi
exit 134
"#,
    );
    assert!(f.run().status.success());
    let launches = std::fs::read_to_string(f.p("tmp/launches")).unwrap();
    assert!(launches.lines().nth(3).unwrap().contains("--safe"));
    let ids = std::fs::read_to_string(f.p("tmp/ids")).unwrap();
    let unique: std::collections::HashSet<_> = ids.lines().collect();
    assert_eq!(unique.len(), 4);
    assert!(unique.iter().all(|id| id.starts_with("00000000-0000-4000-8000-000000000001:")));
}

#[test]
fn stale_markers_distinguish_same_boot_from_power_loss() {
    for (tag, marker, expected) in
        [("power-loss", "previous-boot:run", 0), ("same-boot", "00000000-0000-4000-8000-000000000001:run", 1)]
    {
        let f = Fake::new(tag);
        f.write("oos/oos-ui.running", marker);
        clean_ui(&f);
        assert!(f.run().status.success());
        assert_eq!(ledger(&f).lines().filter(|l| l.starts_with("10000 ")).count(), expected, "{}", tag);
    }
}

#[test]
fn recovery_subprocesses_have_timeouts_and_failure_selects_safe_once() {
    for op in ["crash", "mode", "count", "clear"] {
        let f = Fake::new(&format!("timeout-{}", op));
        f.exec(
            "bin/timeout",
            r#"#!/bin/sh
[ "$1" != -s ] || shift 2
echo "$1 $2 $3" >> "$OOS_UI_TMP/timeouts"
bound=$1; shift
case "$OOS_FAIL_OP:$bound:$2" in
 crash:20:*) exit 124 ;;
 mode:5:mode|count:5:count|clear:5:clear)
 if [ ! -e "$OOS_UI_TMP/failed" ]; then touch "$OOS_UI_TMP/failed"; exit 124; fi ;;
esac
exec "$@"
"#,
        );
        f.exec(
            "bin/ui",
            r#"#!/bin/sh
echo "$*" >> "$OOS_UI_TMP/launches"
n=$(wc -l < "$OOS_UI_TMP/launches")
if [ "$n" -eq 3 ]; then touch "$OOS_UI_TMP/oos-ui.off"; exit 0; fi
[ "$OOS_FAIL_OP" != clear ] || echo '3601.0 0.0' > "$OOS_UPTIME"
[ "$OOS_FAIL_OP" != count ] || exit 1
exit 0
"#,
        );
        let out = f.command("oos-ui-loop").env("PATH", path(&f)).env("OOS_FAIL_OP", op).output().unwrap();
        assert!(out.status.success(), "{} {:?}", op, out);
        let launches = std::fs::read_to_string(f.p("tmp/launches")).unwrap();
        let lines: Vec<_> = launches.lines().collect();
        let safe = if matches!(op, "crash" | "mode") { 0 } else { 1 };
        assert!(lines[safe].contains("--safe"), "{} {}", op, launches);
        assert!(!lines[safe + 1].contains("--safe"), "{} {}", op, launches);
        let timeouts = std::fs::read_to_string(f.p("tmp/timeouts")).unwrap();
        assert!(timeouts.lines().any(|l| l.starts_with("20 ")));
        assert!(timeouts.lines().any(|l| l.starts_with("5 ")));
    }
}

#[test]
fn previous_boot_watchdog_evidence_counts_once() {
    for (tag, reset, expected) in [("wdt-evidence", "32", 1), ("no-reset", "0", 0), ("invalid-reset", "unavailable", 0)]
    {
        let f = Fake::new(tag);
        f.write("oos/oos-ui.running", "previous-boot:run");
        f.write("bootstatus", reset);
        clean_ui(&f);
        for _ in 0..2 {
            let _ = std::fs::remove_file(f.p("tmp/oos-ui.off"));
            assert!(f
                .command("oos-ui-loop")
                .env("OOS_WATCHDOG_BOOTSTATUS", f.p("bootstatus"))
                .output()
                .unwrap()
                .status
                .success());
        }
        assert_eq!(ledger(&f).lines().filter(|l| l.starts_with("10000 ")).count(), expected);
    }
}

#[test]
fn previous_boot_panic_archive_counts_once_before_launch() {
    let f = Fake::new("previous-panic");
    f.write("oos/oos-ui.running", "previous-boot:run");
    std::fs::create_dir(f.p("pstore")).unwrap();
    f.write("pstore/dmesg-ramoops-0", "Panic#1 Part1\nKernel panic - test\n");
    f.exec(
        "bin/df",
        "#!/bin/sh\necho 'Filesystem 1K-blocks Used Available Use% Mounted on'\necho '/dev/x 16384 0 12000 0% data'\n",
    );
    clean_ui(&f);
    let out = f
        .command("oos-ui-loop")
        .env("PATH", path(&f))
        .env("OOS_CRASH_SAVE", image_script("oos-crash-save"))
        .env("OOS_CRASH_MNT", f.p("oos"))
        .env("OOS_PSTORE", f.p("pstore"))
        .env("OOS_KMSG", f.p("kmsg"))
        .env("OOS_BUILD_FILE", f.p("build"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert_eq!(ledger(&f).lines().filter(|l| l.starts_with("10000 ")).count(), 1);
    assert!(ledger(&f).contains("previous-boot:run"));
    assert!(!f.p("pstore/dmesg-ramoops-0").exists());
    std::fs::remove_file(f.p("tmp/oos-ui.off")).unwrap();
    assert!(f.run().status.success());
    assert_eq!(ledger(&f).lines().filter(|l| l.starts_with("10000 ")).count(), 1);
}

#[test]
fn archive_count_timeout_keeps_pstore_and_selects_safe() {
    let f = Fake::new("archive-count-timeout");
    std::fs::create_dir(f.p("pstore")).unwrap();
    f.write("pstore/dmesg-ramoops-0", "Panic#1 Part1\nKernel panic - test\n");
    f.exec(
        "bin/df",
        "#!/bin/sh\necho 'Filesystem 1K-blocks Used Available Use% Mounted on'\necho '/dev/x 16384 0 12000 0% data'\n",
    );
    f.exec(
        "bin/timeout",
        r#"#!/bin/sh
[ "$1" != -s ] || shift 2
shift
case "$2:$6" in count:panic) exit 124 ;; esac
exec "$@"
"#,
    );
    clean_ui(&f);
    let out = f
        .command("oos-ui-loop")
        .env("PATH", path(&f))
        .env("OOS_CRASH_SAVE", image_script("oos-crash-save"))
        .env("OOS_CRASH_MNT", f.p("oos"))
        .env("OOS_PSTORE", f.p("pstore"))
        .env("OOS_KMSG", f.p("kmsg"))
        .env("OOS_BUILD_FILE", f.p("build"))
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    assert!(f.p("pstore/dmesg-ramoops-0").exists());
    assert!(std::fs::read_to_string(f.p("tmp/launches")).unwrap().contains("--safe"));
}
