// SPDX-License-Identifier: GPL-2.0-or-later

//! Run rcS through stock-selection restoration with a card file, never a real device.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn source(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(path)).expect("source readable")
}

#[test]
fn early_panics_hold_before_userspace_can_restore_the_boot_selection() {
    for file in ["rootfs/board/dho/etc/oos-board", "kernel/dts/rk3399-rigol-dho900.dts"] {
        let text = source(file);
        assert!(text.contains("panic=0"), "early panic must halt in {file}");
        assert!(!text.contains("panic=10"), "early reboot loop in {file}");
    }
    assert!(source("kernel/config").contains("CONFIG_PANIC_TIMEOUT=0"));
}

struct Boot(PathBuf);

impl Boot {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("oos-boot-safety-{tag}-{}", std::process::id()));
        fs::create_dir_all(root.join("bin")).expect("fixture bin");
        let f = Self(root);
        f.write("board", "OOS_BOOT_LBA=2\nOOS_BCB_LBA=3\nOOS_PARM_LBA=0\nOOS_PARM_MARKS=layout\n");
        f.exec("bin/sync", "#!/bin/sh\nexit 0\n");
        f.exec("bin/sleep", "#!/bin/sh\nexit 0\n");
        f.exec(
            "bin/dd",
            concat!(
                "#!/usr/bin/env python3\nimport os, pathlib, subprocess, sys\n",
                "args = sys.argv[1:]\nfail = pathlib.Path(os.environ['OOS_FAIL_WRITE'])\n",
                "mode = fail.read_text() if fail.exists() else ''\n",
                "if 'iflag=direct' in args:\n",
                " pathlib.Path(os.environ['OOS_DIRECT_READ']).touch()\n",
                " if mode == 'readback':\n  sys.stdout.write('OOSK')\n  sys.exit(0)\n",
                " args.remove('iflag=direct')\n", // The host dd may lack O_DIRECT; record the requested flag above.
                "if 'of=' + os.environ['OOS_DEV'] in args and mode in ('before', 'after'):\n",
                " if mode == 'after': subprocess.run(['/bin/dd', *args], check=True)\n",
                " sys.exit(1)\n",
                "os.execv('/bin/dd', ['dd', *args])\n",
            ),
        );
        let script = source("rootfs/initramfs/bin/oos-oneshot").replace("/tmp/", &format!("{}/", f.0.display()));
        let shell = std::env::var("OOS_TEST_SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let script = script.replacen("#!/bin/sh", &format!("#!{shell}"), 1);
        f.exec("oneshot", &script);
        // Execute the real early rcS in order, stopping before background services and hardware IO.
        let rcs = source("rootfs/initramfs/etc/rcS");
        let early = rcs
            .split("# 0b)")
            .next()
            .expect("rcS prefix")
            .replace("/bin/oos-oneshot", &f.p("oneshot").display().to_string())
            .replace("/proc/sys/kernel/softlockup_panic", &f.p("softlockup").display().to_string())
            .replace("/proc/sys/kernel/panic", &f.p("panic").display().to_string())
            .replace("/tmp/oos-boot.status", &f.p("status").display().to_string());
        let early = early.replace(
            "(setsid sh -c '/bin/oos-watchdog >> /tmp/oos-watchdog.log 2>&1' < /dev/null > /dev/null 2>&1 &)",
            &format!("echo armed > '{}'", f.p("watchdog-allowed").display()),
        );
        f.exec("rcS", &early);
        f
    }
    fn p(&self, path: &str) -> PathBuf {
        self.0.join(path)
    }
    fn write(&self, path: &str, text: &str) {
        fs::write(self.p(path), text).expect("fixture write");
    }
    fn exec(&self, path: &str, text: &str) {
        self.write(path, text);
        fs::set_permissions(self.p(path), fs::Permissions::from_mode(0o755)).expect("executable fixture");
    }
    fn run(&self, magic: &[u8; 4], table: bool, fail: &str) -> (String, Vec<u8>) {
        let mut card = vec![0; 2048];
        card[..11].copy_from_slice(if table { b"PARM layout" } else { b"BAD! layout" });
        card[1024..1028].copy_from_slice(magic);
        fs::write(self.p("card"), card).expect("card file");
        self.write("panic", "0\n");
        self.write("softlockup", "0\n");
        self.write("status", "");
        let _ = fs::remove_file(self.p("direct-read"));
        let _ = fs::remove_file(self.p("watchdog-allowed"));
        if !fail.is_empty() {
            self.write("fail", fail);
        } else {
            let _ = fs::remove_file(self.p("fail"));
        }
        let out = Command::new(std::env::var("OOS_TEST_SHELL").unwrap_or_else(|_| "sh".into()))
            .arg(self.p("rcS"))
            .env("PATH", format!("{}:{}", self.p("bin").display(), std::env::var("PATH").expect("PATH")))
            .env("OOS_BOARD_FILE", self.p("board"))
            .env("OOS_DEV", self.p("card"))
            .env("OOS_TEST", "1")
            .env("OOS_LOG", self.p("log"))
            .env("OOS_FAIL_WRITE", self.p("fail"))
            .env("OOS_DIRECT_READ", self.p("direct-read"))
            .output()
            .expect("rcS runs");
        assert!(out.status.success(), "{out:?}");
        (fs::read_to_string(self.p("panic")).expect("panic sysctl"), fs::read(self.p("card")).expect("card"))
    }
}

impl Drop for Boot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn panic_restart_is_enabled_only_after_stock_selection_is_confirmed() {
    let f = Boot::new("ordering");
    for (magic, table, fail, safe) in [
        (b"OOSK", true, "", true),
        (b"KRNL", true, "", true),
        (b"OOSK", true, "before", false),
        (b"OOSK", true, "after", false),
        (b"OOSK", true, "readback", false),
        (b"BAD!", true, "", false),
        (b"OOSK", false, "", false),
    ] {
        let (panic, card) = f.run(magic, table, fail);
        assert_eq!(panic.trim(), if safe { "10" } else { "0" }, "{magic:?}, table {table}, fail {fail}");
        assert_eq!(&card[1024..1028], if safe || matches!(fail, "after" | "readback") { b"KRNL" } else { magic });
        assert_eq!(f.p("watchdog-allowed").exists(), safe);
        let soft = fs::read_to_string(f.p("softlockup")).expect("softlockup sysctl");
        assert_eq!(soft.trim(), if safe { "1" } else { "0" }, "kexec starts keep the old command line");
        let status = fs::read_to_string(f.p("status")).expect("boot status");
        assert_eq!(status.contains("watchdog and panic restart are off"), !safe, "{status}");
    }
}

#[test]
fn restoration_requests_a_direct_read_back() {
    let f = Boot::new("direct");
    let (panic, _) = f.run(b"OOSK", true, "");
    assert!(f.p("direct-read").exists(), "restore must request a read without the page cache");
    assert_eq!(panic.trim(), "10");
}

#[test]
fn a_read_back_mismatch_keeps_automatic_restarts_disabled() {
    let f = Boot::new("mismatch");
    let (panic, _) = f.run(b"OOSK", true, "readback");
    assert_eq!(panic.trim(), "0");
    assert!(!f.p("watchdog-allowed").exists());
}
