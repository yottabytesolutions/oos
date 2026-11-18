// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

#[cfg(test)]
mod cuts;
#[cfg(test)]
mod lock;
#[cfg(test)]
mod review;
#[cfg(test)]
mod round2;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug)]
pub struct Fake {
    root: PathBuf,
}

impl Fake {
    pub fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("oos-safe-{}-{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["oos", "tmp", "bin", "host-bin"] {
            std::fs::create_dir_all(root.join(d)).expect("test fixture operation succeeds");
        }
        let f = Self { root };
        f.write("now", "10000\n");
        f.write("boot_id", "00000000-0000-4000-8000-000000000001\n");
        f.write("uptime", "0.0 0.0\n");
        f.write("tmp/oos-ui.args", "--demo\n");
        // Host macOS sync flushes the whole machine. Match the image's per-path
        // sync semantics so unrelated builds cannot trigger a supervisor timeout.
        f.exec(
            "host-bin/sync",
            r#"#!/bin/sh
exec python3 -S - "$@" <<'PY_SYNC'
import os, sys
for path in sys.argv[1:]:
    fd = os.open(path, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)
PY_SYNC
"#,
        );
        f.exec("bin/crash-save", "#!/bin/sh\nexit 0\n");
        // Supply Linux proc identities to the actual POSIX shell script on non-Linux hosts too.
        f.exec(
            "host-bin/test-shell",
            r#"#!/bin/sh
mkdir -p "$OOS_PROC/$$"
printf '%s (fixture ) process) S 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 12345 0\n' "$$" > "$OOS_PROC/$$/stat"
exec "$OOS_REAL_SHELL" "$@"
"#,
        );
        f.exec("host-bin/test-exits", "#!/bin/sh\nexec \"$OOS_TEST_SHELL_WRAPPER\" \"$OOS_EXITS_SCRIPT\" \"$@\"\n");
        f
    }

    pub fn p(&self, p: &str) -> PathBuf {
        self.root.join(p)
    }

    pub fn write(&self, p: &str, text: &str) {
        std::fs::write(self.p(p), text).expect("test fixture operation succeeds");
    }

    pub fn exec(&self, p: &str, text: &str) {
        self.write(p, text);
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(self.p(p), std::fs::Permissions::from_mode(0o755))
            .expect("test fixture operation succeeds");
    }

    pub fn command(&self, script: &str) -> Command {
        let mut c = Command::new(self.p("host-bin/test-shell"));
        c.arg(image_script(script))
            .env("OOS_REAL_SHELL", std::env::var("OOS_TEST_SHELL").unwrap_or_else(|_| "sh".into()))
            .env("OOS_PROC", self.p("proc"))
            .env("OOS_TEST_SHELL_WRAPPER", self.p("host-bin/test-shell"))
            .env("OOS_EXITS_SCRIPT", image_script("oos-ui-exits"))
            .env("PATH", format!("{}:{}", self.p("host-bin").display(), std::env::var("PATH").unwrap_or_default()))
            .env("OOS_UI_DATA", self.p("oos"))
            .env("OOS_UI_TMP", self.p("tmp"))
            .env("OOS_UI_BIN", self.p("bin/ui"))
            .env("OOS_CRASH_SAVE", self.p("bin/crash-save"))
            .env("OOS_UI_EXITS", self.p("host-bin/test-exits"))
            .env("OOS_NOW_FILE", self.p("now"))
            .env("OOS_BOOT_ID", self.p("boot_id"))
            .env("OOS_UPTIME", self.p("uptime"))
            .env("OOS_UI_DELAY", "0")
            .env("OOS_TEST", "1");
        c
    }

    pub fn count(&self, id: &str, rc: &str) {
        let out =
            self.command("oos-ui-exits").args(["count", id, rc, id]).output().expect("test fixture operation succeeds");
        assert!(out.status.success(), "{:?}", out);
    }

    pub fn mode(&self) -> String {
        let out = self.command("oos-ui-exits").arg("mode").output().expect("test fixture operation succeeds");
        assert!(out.status.success(), "{:?}", out);
        String::from_utf8(out.stdout).expect("test fixture operation succeeds").trim().into()
    }

    pub fn run(&self) -> Output {
        self.command("oos-ui-loop").output().expect("test fixture operation succeeds")
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn image_script(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../rootfs/initramfs/bin").join(name)
}
