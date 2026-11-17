// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The image's self-heal scripts on the host (task 0045, ADR 0015 "Kernel panics"): `oos-crash-save` against a fake
//! pstore and a fake `/oos` whose `df` reports a 16 MiB partition, and the `oos-watchdog` wrapper with a kicker that
//! exits at once. The scripts are POSIX sh; on the unit they run under busybox.

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

fn script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../rootfs/initramfs/bin").join(name)
}

/// A fresh directory with `pstore/`, `oos/` and `bin/` (the `df` shim).
struct Fake {
    root: PathBuf,
}

impl Fake {
    fn new(tag: &str) -> Fake {
        let root = std::env::temp_dir().join(format!("oos-selfheal-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["pstore", "oos", "bin"] {
            std::fs::create_dir_all(root.join(d)).expect("temp dir is writable");
        }
        let fake = Fake { root };
        fake.free_kb(12 * 1024);
        for cmd in ["cp", "mv", "rm", "sync"] {
            fake.shim(cmd);
        }
        std::fs::write(fake.p("boot_id"), "00000000-0000-4000-8000-000000000001\n").expect("boot_id");
        std::fs::write(fake.p("oos-build"), "test build\n").expect("build");
        // oos-ui-exits reads its own Linux proc identity for the lock; supply one on non-Linux hosts too.
        write_exec(
            &fake.p("bin/test-exits"),
            "#!/bin/sh\nmkdir -p \"$OOS_PROC/$$\"\n\
             printf '%s (fixture ) process) S 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 12345 0\\n' \"$$\" > \"$OOS_PROC/$$/stat\"\n\
             exec sh \"$OOS_EXITS_SCRIPT\" \"$@\"\n",
        );
        fake
    }
    /// A pass-through `cmd` that fails (`fail N`) or corrupts its last argument (`corrupt N`) on its N-th call, as
    /// `plan` says.
    fn shim(&self, cmd: &str) {
        let real = Command::new("sh").arg("-c").arg(format!("command -v {}", cmd)).output().expect("sh runs");
        let mut real = String::from_utf8_lossy(&real.stdout).trim().to_string();
        if cmd == "sync" {
            // macOS sync flushes the entire host. Match the image's per-path operation so
            // unrelated builds do not consume the counter's five-second process deadline.
            let helper = self.p("bin/host-sync");
            write_exec(&helper, "#!/bin/sh\nexec python3 - \"$@\" <<'PY_SYNC'\nimport os, sys\nfor path in sys.argv[1:]:\n    fd = os.open(path, os.O_RDONLY)\n    try: os.fsync(fd)\n    finally: os.close(fd)\nPY_SYNC\n");
            real = helper.to_string_lossy().into_owned();
        }
        assert!(real.starts_with('/'), "no {} on the host", cmd);
        let (count, plan) = (self.p(&format!("count-{}", cmd)), self.p(&format!("plan-{}", cmd)));
        let body = format!(
            "#!/bin/sh\nc=$(cat '{count}' 2>/dev/null || echo 0); c=$((c + 1)); echo $c > '{count}'\n\
             case \"$(cat '{plan}' 2>/dev/null)\" in\n\
             \"fail $c\") exit 1 ;;\n\
             \"corrupt $c\") '{real}' \"$@\"; r=$?; eval last=\\${{$#}}; echo x >> \"$last\"; exit $r ;;\n\
             esac\nexec '{real}' \"$@\"\n",
            count = count.display(),
            plan = plan.display(),
            real = real
        );
        write_exec(&self.p(&format!("bin/{}", cmd)), &body);
    }
    /// The next run of the scripts: `cmd` acts as `plan` says (`fail N`, `corrupt N`, or "" for none).
    fn plan(&self, cmd: &str, plan: &str) {
        let _ = std::fs::remove_file(self.p(&format!("count-{}", cmd)));
        std::fs::write(self.p(&format!("plan-{}", cmd)), plan).expect("plan written");
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
    /// `df -k` of a 16 MiB `/oos` with `kb` KiB available.
    fn free_kb(&self, kb: u64) {
        let df = format!(
            "#!/bin/sh\necho 'Filesystem 1K-blocks Used Available Use% Mounted on'\necho \"/dev/x 16384 0 {} 0% $2\"\n",
            kb
        );
        write_exec(&self.p("bin/df"), &df);
    }
    fn record(&self, name: &str, body: &[u8], mtime_s: u64) {
        let f = self.p(&format!("pstore/{}", name));
        std::fs::write(&f, body).expect("pstore file");
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(mtime_s);
        std::fs::File::options().write(true).open(&f).and_then(|h| h.set_modified(t)).expect("set mtime");
    }
    fn panic(&self, text: &str) {
        self.record("dmesg-ramoops-0", format!("Panic#1 Part1\n<0>[ 1.0] Kernel panic - {}\n", text).as_bytes(), 1_000);
        self.record("console-ramoops-0", &vec![b'c'; 200_000], 1_000);
    }
    fn env(&self, c: &mut Command) {
        let path = format!("{}:{}", self.p("bin").display(), std::env::var("PATH").unwrap_or_default());
        c.env("PATH", path)
            .env("OOS_TEST", "1")
            .env("OOS_UI_TMP", &self.root)
            .env("OOS_PSTORE", self.p("pstore"))
            .env("OOS_CRASH_MNT", self.p("oos"))
            .env("OOS_KMSG", self.p("kmsg"))
            .env("OOS_BOOT_ID", self.p("boot_id"))
            .env("OOS_BUILD_FILE", self.p("oos-build"))
            .env("OOS_PROC", self.p("proc"))
            .env("OOS_EXITS_SCRIPT", script("oos-ui-exits"))
            .env("OOS_UI_EXITS", self.p("bin/test-exits"));
    }
    fn save(&self) -> String {
        let mut c = Command::new("sh");
        c.arg(script("oos-crash-save"));
        self.env(&mut c);
        let out = c.output().expect("sh runs");
        assert!(out.status.success(), "oos-crash-save never fails the start");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
    fn archives(&self) -> Vec<String> {
        let mut v: Vec<String> = match std::fs::read_dir(self.p("oos/crash")) {
            Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
            Err(_) => Vec::new(),
        };
        v.sort();
        v
    }
    fn committed(&self) -> Vec<String> {
        self.archives().into_iter().filter(|n| !n.starts_with('.')).collect()
    }
    fn partials(&self) -> usize {
        self.archives().iter().filter(|n| n.starts_with(".partial-")).count()
    }
    fn pstore(&self) -> usize {
        std::fs::read_dir(self.p("pstore")).expect("pstore dir").count()
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).expect("shim written");
    let mut perm = std::fs::metadata(path).expect("shim exists").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perm, 0o755);
    std::fs::set_permissions(path, perm).expect("shim executable");
}

fn dir_kb(p: &Path) -> u64 {
    let out = Command::new("du").arg("-sk").arg(p).output().expect("du runs");
    String::from_utf8_lossy(&out.stdout).split_whitespace().next().and_then(|s| s.parse().ok()).expect("du prints KiB")
}

#[test]
fn a_panic_is_archived_bounded_and_pstore_cleared() {
    let f = Fake::new("panic");
    f.panic("sysrq");
    let out = f.save();
    assert!(out.contains("reason panic"), "{}", out);
    let a = f.archives();
    assert_eq!(a.len(), 1, "{:?}", a);
    let dir = f.p("oos/crash").join(&a[0]);
    let dmesg = std::fs::read_to_string(dir.join("dmesg-ramoops-0")).expect("dmesg archived");
    assert!(dmesg.starts_with("Panic#1"));
    let console = std::fs::metadata(dir.join("console-ramoops-0")).expect("console archived").len();
    assert_eq!(console, 65536, "the console's last 64 KiB");
    assert_eq!(f.pstore(), 0, "the archived records are cleared");
}

#[test]
fn an_interrupted_start_does_not_archive_twice() {
    let f = Fake::new("again");
    f.panic("one");
    f.save();
    // the next start was cut after the archive, before the clear: pstore still holds the same records
    f.panic("one");
    let out = f.save();
    assert!(out.contains("already holds"), "{}", out);
    assert_eq!(f.archives().len(), 1);
    assert_eq!(f.pstore(), 0);
    // a different panic with the same pstore id and time is a new archive
    f.panic("two");
    f.save();
    assert_eq!(f.archives().len(), 2);
}

#[test]
fn console_records_alone_archive_nothing() {
    let f = Fake::new("console");
    f.record("console-ramoops-0", b"clean reboot\n", 1_000);
    f.record("pmsg-ramoops-0", b"x", 1_000);
    let out = f.save();
    assert!(out.contains("no crash record"), "{}", out);
    assert!(f.archives().is_empty());
    assert_eq!(f.pstore(), 2, "console and pmsg stay");
    assert_eq!(exits(&f), 0);
}

#[test]
fn the_budget_holds_and_the_newest_stays() {
    let f = Fake::new("budget");
    for (i, n) in ["1-a", "2-b", "3-c", "4-d"].iter().enumerate() {
        let d = f.p("oos/crash").join(n);
        std::fs::create_dir_all(&d).expect("old archive");
        std::fs::write(d.join("dmesg-ramoops-0"), vec![b'x'; 300_000 + i]).expect("old record");
    }
    f.panic("new");
    f.save();
    let a = f.archives();
    assert!(dir_kb(&f.p("oos/crash")) <= 1024, "{:?}", a);
    assert!(a.iter().any(|n| n.starts_with("5-")), "{:?}", a);
    assert!(!a.contains(&"1-a".to_string()), "the oldest goes first: {:?}", a);
}

#[test]
fn the_reserve_skips_without_pruning_and_keeps_pstore() {
    let f = Fake::new("reserve");
    std::fs::create_dir_all(f.p("oos/crash/1-a")).expect("old archive");
    f.free_kb(4100);
    f.panic("tight");
    let out = f.save();
    assert!(out.contains("not archived"), "{}", out);
    assert_eq!(f.archives(), vec!["1-a".to_string()]);
    assert_eq!(f.pstore(), 2);
}

#[test]
fn a_failed_archive_keeps_pstore_and_an_interrupted_one_is_kept() {
    let f = Fake::new("fail");
    std::fs::write(f.p("oos/crash"), b"not a directory").expect("blocker");
    f.panic("blocked");
    assert!(f.save().contains("not archived"));
    assert_eq!(f.pstore(), 2);
    std::fs::remove_file(f.p("oos/crash")).expect("blocker removed");
    std::fs::create_dir_all(f.p("oos/crash/.tmp")).expect("left by a cut save");
    f.save();
    let a = f.archives();
    assert!(a.contains(&".partial-1".to_string()), "{:?}", a);
    assert!(a.iter().any(|n| n.starts_with("2-")), "{:?}", a);
}

#[test]
fn records_without_a_panic_or_oops_are_left_alone() {
    let f = Fake::new("other");
    f.record("dmesg-ramoops-0", b"Emergency#1 Part1\nreboot\n", 1_000);
    let out = f.save();
    assert!(out.contains("without a panic or oops"), "{}", out);
    assert!(f.archives().is_empty());
    assert_eq!(f.pstore(), 1);
}

#[test]
fn a_failed_file_fsync_keeps_pstore_and_commits_nothing() {
    let f = Fake::new("fsync");
    f.panic("p");
    f.plan("sync", "fail 1");
    assert!(f.save().contains("fsync in"));
    assert_eq!((f.pstore(), f.committed().len()), (2, 0));
    f.plan("sync", "");
    f.save();
    assert_eq!((f.pstore(), f.committed().len(), f.partials()), (0, 1, 1));
}

#[test]
fn a_retry_after_a_failed_directory_fsync_syncs_again_before_clearing() {
    let f = Fake::new("dirsync");
    f.panic("p");
    f.plan("sync", "fail 2");
    assert!(f.save().contains("fsync of"));
    assert_eq!((f.pstore(), f.committed().len()), (2, 1), "renamed, not yet durable");
    // the retry finds the archive; its directory fsync fails again, so pstore must stay
    f.plan("sync", "fail 2");
    assert!(f.save().contains("not archived"));
    assert_eq!(f.pstore(), 2);
    f.plan("sync", "");
    assert!(f.save().contains("already holds"));
    assert_eq!((f.pstore(), f.committed().len()), (0, 1));
}

#[test]
fn a_copy_that_reads_back_wrong_keeps_pstore() {
    let f = Fake::new("readback");
    f.panic("p");
    f.plan("cp", "corrupt 1");
    assert!(f.save().contains("read-back"));
    assert_eq!((f.pstore(), f.committed().len()), (2, 0));
}

#[test]
fn a_metadata_copy_that_reads_back_wrong_keeps_pstore() {
    // cp calls: 1 the dmesg record, 2 boot_id, 3 the build
    for call in ["corrupt 2", "corrupt 3"] {
        let f = Fake::new("metadata");
        f.panic("p");
        f.plan("cp", call);
        assert!(f.save().contains("read-back"), "{}", call);
        assert_eq!((f.pstore(), f.committed().len()), (2, 0), "{}", call);
    }
}

#[test]
fn an_archive_that_no_longer_matches_is_set_aside_and_redone() {
    let f = Fake::new("mismatch");
    f.panic("p");
    f.plan("sync", "fail 2");
    f.save();
    let old = f.committed()[0].clone();
    std::fs::write(f.p("oos/crash").join(&old).join("dmesg-ramoops-0"), b"torn").expect("damage");
    f.plan("sync", "");
    f.save();
    let new = f.committed();
    assert_eq!((f.pstore(), new.len(), f.partials()), (0, 1, 1), "{:?}", f.archives());
    assert_ne!(new[0], old);
    let d = std::fs::read_to_string(f.p("oos/crash").join(&new[0]).join("dmesg-ramoops-0")).expect("redone");
    assert!(d.starts_with("Panic#1"));
}

#[test]
fn a_failed_rename_or_clear_is_repeated_without_a_second_archive() {
    let f = Fake::new("mvrm");
    f.panic("p");
    f.plan("mv", "fail 1");
    assert!(f.save().contains("rename"));
    assert_eq!((f.pstore(), f.committed().len()), (2, 0));
    f.plan("mv", "");
    f.plan("rm", "fail 1");
    f.save();
    assert_eq!(f.committed().len(), 1);
    assert!(f.pstore() > 0, "the first clear failed");
    f.plan("rm", "");
    assert!(f.save().contains("already holds"));
    assert_eq!((f.pstore(), f.committed().len()), (0, 1));
}

#[test]
fn repeated_interrupted_saves_keep_one_partial() {
    let f = Fake::new("partials");
    f.panic("p");
    for _ in 0..4 {
        f.plan("cp", "fail 1");
        assert!(f.save().contains("copy of"));
    }
    assert!(f.partials() <= 1, "{:?}", f.archives());
    assert_eq!((f.pstore(), f.committed().len()), (2, 0));
}

#[test]
fn a_missing_boot_id_keeps_pstore() {
    let f = Fake::new("bootid");
    f.panic("p");
    std::fs::remove_file(f.p("boot_id")).expect("boot_id removed");
    assert!(f.save().contains("boot_id not recorded"));
    assert_eq!((f.pstore(), f.committed().len()), (2, 0));
}

#[test]
fn the_watchdog_wrapper_restarts_an_exiting_kicker_with_growing_delays() {
    let f = Fake::new("wd");
    // Record requested delays; advancing the fake sleep is immediate.
    write_exec(&f.p("bin/sleep"), &format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n", f.p("delays").display()));
    write_exec(&f.p("bin/watchdog"), "#!/bin/sh\nexit 3\n");
    let log_path = f.p("wd.log");
    let log = std::fs::File::create(&log_path).expect("log file");
    let mut c = Command::new("sh");
    c.arg(script("oos-watchdog")).env("OOS_WATCHDOG_DEV", "/dev/null").env("OOS_DEBUGFS", f.p("debug"));
    c.env("OOS_CMDLINE", "/dev/null").stdin(Stdio::null()).stdout(log).stderr(Stdio::null());
    f.env(&mut c);
    let mut child = c.spawn().expect("sh runs");
    support::wait_for("watchdog requested third restart", || {
        std::fs::read_to_string(&log_path).is_ok_and(|text| text.contains("again in 4 s"))
    });
    child.kill().expect("the wrapper runs until killed");
    child.wait().expect("reaped");
    let text = std::fs::read_to_string(&log_path).expect("log");
    assert!(text.contains("before open: hw unknown") && text.contains("exited (3)"), "{}", text);
    let at = |w: &str| text.find(w).unwrap_or(usize::MAX);
    assert!(at("again in 1 s") < at("again in 2 s") && at("again in 2 s") < at("again in 4 s"), "{}", text);
    let delays = std::fs::read_to_string(f.p("delays")).unwrap();
    assert_eq!(delays.lines().take(2).collect::<Vec<_>>(), ["1", "2"]);
}

fn exits(f: &Fake) -> usize {
    std::fs::read_to_string(f.p("oos/oos-ui.exits"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.as_bytes().first().is_some_and(u8::is_ascii_digit))
        .count()
}

#[test]
fn a_panic_is_counted_once() {
    let f = Fake::new("count-panic");
    f.panic("one");
    f.save();
    assert_eq!(exits(&f), 1);
    f.panic("one");
    f.save();
    assert_eq!(exits(&f), 1);
}

#[test]
fn panic_and_running_marker_are_one_exit() {
    let f = Fake::new("marker-panic");
    std::fs::write(f.p("oos/oos-ui.running"), "boot:run1\n").unwrap();
    f.panic("one");
    f.save();
    let mut c = Command::new(f.p("bin/test-exits"));
    c.args(["count", "boot:run1", "1", "boot:run1"]);
    f.env(&mut c);
    assert!(c.env("OOS_UI_DATA", f.p("oos")).status().unwrap().success());
    assert_eq!(exits(&f), 1);
}

#[test]
fn counter_failure_keeps_pstore() {
    let f = Fake::new("count-fail");
    f.panic("one");
    std::fs::create_dir(f.p("oos/oos-ui.exits.new")).unwrap();
    assert!(f.save().contains("counter not committed"));
    assert_eq!((f.pstore(), f.committed().len(), exits(&f)), (2, 1, 0));
    std::fs::remove_dir(f.p("oos/oos-ui.exits.new")).unwrap();
    f.save();
    assert_eq!((f.pstore(), f.committed().len(), exits(&f)), (0, 1, 1));
}

#[test]
fn clear_retry_does_not_count_twice() {
    let f = Fake::new("count-clear");
    f.panic("one");
    f.plan("rm", "fail 1");
    f.save();
    assert_eq!((f.pstore(), exits(&f)), (1, 1));
    f.plan("rm", "");
    f.save();
    assert_eq!((f.pstore(), exits(&f)), (0, 1));
}

#[test]
fn an_old_archive_retry_does_not_consume_a_new_runs_marker() {
    let f = Fake::new("new-run");
    std::fs::write(f.p("oos/oos-ui.running"), "boot:old\n").unwrap();
    f.panic("one");
    f.plan("rm", "fail 1");
    f.save();
    assert_eq!(exits(&f), 1);
    std::fs::write(f.p("oos/oos-ui.running"), "boot:new\n").unwrap();
    f.plan("rm", "");
    f.save();
    let mut c = Command::new(f.p("bin/test-exits"));
    c.args(["count", "boot:new", "1", "boot:new"]);
    f.env(&mut c);
    assert!(c.env("OOS_UI_DATA", f.p("oos")).status().unwrap().success());
    assert_eq!(exits(&f), 2);
}

#[test]
fn rearchiving_damaged_records_keeps_the_original_counter_identity() {
    let f = Fake::new("count-damaged");
    f.panic("one");
    f.plan("rm", "fail 1");
    f.save();
    assert_eq!((f.pstore(), exits(&f)), (1, 1));
    let old = f.committed()[0].clone();
    std::fs::write(f.p("oos/crash").join(old).join("dmesg-ramoops-0"), b"damaged").unwrap();
    f.plan("rm", "");
    f.save();
    assert_eq!((f.pstore(), exits(&f)), (0, 1));
}

#[test]
fn an_old_panic_retry_preserves_the_newer_run_receipt() {
    let f = Fake::new("count-receipt");
    std::fs::write(f.p("oos/oos-ui.running"), "boot:old\n").unwrap();
    f.panic("one");
    f.plan("rm", "fail 1");
    f.save();
    let count = || {
        let mut c = Command::new(f.p("bin/test-exits"));
        c.args(["count", "boot:new", "1", "boot:new"]);
        f.env(&mut c);
        assert!(c.env("OOS_UI_DATA", f.p("oos")).status().unwrap().success());
    };
    count();
    f.plan("rm", "");
    f.save();
    count();
    assert_eq!(exits(&f), 2);
}
