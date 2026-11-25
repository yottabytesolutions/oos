# SPDX-License-Identifier: GPL-2.0-or-later
"""Exercise bounded shutdown and recovery without devices or elapsed-time assertions."""
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SHELL = shlex.split(os.environ.get("OOS_TEST_SHELL", "bash"))


class DataShutdown(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        self.trace = self.root / "trace"
        self.kmsg = self.root / "kmsg"
        self.marker = self.root / "off"
        self.closing = self.root / "closing"
        self.mounts = self.root / "proc" / "mounts"
        self.trace.touch()
        self.kmsg.touch()
        self.mounts.parent.mkdir()
        self.mounts.write_text("/dev/example /oos ext4 rw,noatime 0 0\n")
        os.mkfifo(self.root / "blocked")
        source = (ROOT / "rootfs/initramfs/bin/oos-shutdown-data").read_text()
        paths = {
            "/proc/": self.mounts.parent.as_posix() + "/",
            "/dev/kmsg": self.kmsg,
            "/tmp/oos-ui.off": self.marker,
            "/tmp/oos-data.closing": self.closing,
            "/tmp/oos-shutdown.": self.root / "shutdown.",
        }
        source = source.replace(" > /dev/kmsg", " >> /dev/kmsg")
        source = re.sub("|".join(map(re.escape, paths)), lambda m: str(paths[m[0]]), source)
        self.worker_pid = self.root / "worker.pid"
        source = source.replace('setsid "$0"', '"$STUBS/setsid" "$0"')
        source = source.replace('    shift\n    case $1 in', '    printf "%s\\n" "$$" > "$WORKER_PID"\n    shift\n    case $1 in')
        stubs = ("killall", "pidof", "sync", "umount", "mount", "sleep")
        # The image's standalone ash can prefer applets to PATH, so point test functions at fixtures.
        shim = "".join(f'{name}() {{ "$STUBS/{name}" "$@"; }}\n' for name in stubs)
        shim += '''
kill() {
    if [ "$1" = -0 ] && [ "$2" = "-42" ]; then
        printf 'probe %s\n' "$2" >> "$TRACE"
        [ "$LATE_WORKER" = 1 ] && [ -s "$MOUNTS" ]
    else
        command kill "$@"
    fi
}
'''
        self.script = self.root / "shutdown"
        self.script.write_text(f"#!/bin/sh\n{shim}\n" + source)
        self.script.chmod(0o700)

        if Path(SHELL[0]).name == "busybox":
            self.stub("setsid", f'exec {shlex.quote(SHELL[0])} setsid {shlex.join(SHELL)} "$@"\n')
        else:
            # macOS has no setsid applet.
            self.stub(
                "setsid",
                "import os, sys\nos.setsid()\nos.execvp(sys.argv[1], sys.argv[1:])\n",
                sys.executable,
            )
        self.stub("killall", '[ -f "$MARKER" ] || exit 2\nprintf "killall %s\n" "$*" >> "$TRACE"\n')
        self.stub("pidof", '''printf "pidof %s\n" "$*" >> "$TRACE"
[ "$STUBBORN_UI" = 1 ] && ! grep -q 'killall -KILL' "$TRACE"
''')
        self.stub("sleep", '''
if [ "$LATE_WORKER" = 1 ]; then
    printf 'late umount completes\n' >> "$TRACE"
    : > "$MOUNTS"
fi
''')
        self.stub("sync", '''[ -d "$CLOSING" ] || exit 2
printf 'sync\n' >> "$TRACE"
if [ "$TIMEOUT_AT" = sync ]; then
    printf 'IO in progress\n' >&3
    exec cat "$BLOCKED"
fi
exit "$SYNC_EXIT"
''')
        self.stub("umount", '''printf "umount %s\n" "$*" >> "$TRACE"
if [ "$TIMEOUT_AT" = umount ]; then
    printf 'IO in progress\n' >&3
    exec cat "$BLOCKED"
fi
[ "$UMOUNT_EXIT" = 0 ] || { echo "Device or resource busy" >&2; exit "$UMOUNT_EXIT"; }
: > "$MOUNTS"
''')
        self.stub("mount", '''printf "mount %s\n" "$*" >> "$TRACE"
case "$*" in
    '-o remount,ro /oos')
        [ "$MOUNT_EXIT" = 0 ] || { echo "$MOUNT_ERROR" >&2; exit "$MOUNT_EXIT"; }
        echo '/dev/example /oos ext4 ro,noatime 0 0' > "$MOUNTS" ;;
    '-o remount,rw,noatime /oos') : > "$MOUNTS" ;;
    '-t ext4 -o rw,noatime /dev/example /oos')
        if [ "$MOUNT_UPDATES_STATE" = 1 ]; then
            echo '/dev/example /oos ext4 rw,noatime 0 0' > "$MOUNTS"
        fi ;;
esac
exit "$MOUNT_EXIT"
''')

    def stub(self, name, body, interpreter="/bin/sh"):
        path = self.root / name
        path.write_text(f"#!{interpreter}\n{body}")
        path.chmod(0o700)

    def run_helper(self, mode="ui-drained", *args, code=0, **overrides):
        if mode == "restore" and not self.closing.exists():
            self.closing.mkdir()
        env = {
            **os.environ,
            "STUBS": str(self.root),
            "TRACE": str(self.trace),
            "WORKER_PID": str(self.worker_pid),
            "KMSG": str(self.kmsg),
            "MARKER": str(self.marker),
            "CLOSING": str(self.closing),
            "MOUNTS": str(self.mounts),
            "BLOCKED": str(self.root / "blocked"),
            "UMOUNT_EXIT": "0",
            "MOUNT_EXIT": "0",
            "MOUNT_ERROR": "Input/output error",
            "MOUNT_UPDATES_STATE": "1",
            "SYNC_EXIT": "0",
            "TIMEOUT_AT": "",
            "LATE_WORKER": "0",
            "STUBBORN_UI": "0",
            "EXPECTED_BOUND": "5" if mode == "stop-ui" else "2",
            **overrides,
        }
        # Deliver the simulated timeout after sync signals that it is blocked.
        # The outcome and asserted bound are independent of scheduling.
        driver = '''
read() {
    if [ "$2" != -t ]; then command read "$@"; return; fi
    [ "$1" = -r ] && [ "$3" = "$EXPECTED_BOUND" ] || return 2
    command read -r "$4" || return
    [ "$result" != "IO in progress" ]
}
. "$0"
'''
        result = subprocess.run([*SHELL, "-c", driver, str(self.script), mode, *args],
                                env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, code, result.stderr)
        self.assertEqual(list(self.root.glob("shutdown.*")), [], "EXIT trap left a FIFO directory")
        return self.trace.read_text().splitlines(), self.kmsg.read_text().splitlines(), result.stdout

    def test_stops_ui_before_sync_and_unmount(self):
        calls, logs, _ = self.run_helper("stop-ui")
        self.assertTrue(self.marker.exists())
        self.assertEqual(calls, ["killall -TERM oos-ui", "pidof oos-ui", "sync", "mount -o remount,ro /oos", "umount /oos"])
        self.assertEqual(logs, [])

    def test_unresponsive_ui_is_killed_before_sync(self):
        calls, _, _ = self.run_helper("stop-ui", STUBBORN_UI="1")
        self.assertEqual(calls, ["killall -TERM oos-ui", *(["pidof oos-ui"] * 30),
                                 "killall -KILL oos-ui", "pidof oos-ui", "sync", "mount -o remount,ro /oos", "umount /oos"])

    def test_drained_ui_does_not_disable_the_supervisor(self):
        calls, logs, _ = self.run_helper()
        self.assertFalse(self.marker.exists())
        self.assertTrue(self.closing.exists())
        self.assertEqual(calls, ["sync", "mount -o remount,ro /oos", "umount /oos"])
        self.assertEqual(logs, [])

    def test_busy_unmount_stays_read_only(self):
        calls, logs, _ = self.run_helper(code=1, UMOUNT_EXIT="1")
        self.assertEqual(calls, ["sync", "mount -o remount,ro /oos", "umount /oos"])
        self.assertIn("Device or resource busy", logs[0])
        self.assertIn("leaving /oos read-only", logs[0])
        self.assertIn("ext4 ro,noatime", self.mounts.read_text())
        self.assertTrue(self.closing.exists())

    def test_missing_data_mount_leaves_other_mounts_alone(self):
        self.mounts.write_text("/dev/example /oos-extra ext4 rw 0 0\n")
        calls, logs, receipt = self.run_helper()
        self.assertEqual(calls, ["sync"])
        self.assertEqual(logs, ["oos-shutdown-data: /oos already unmounted"])
        self.assertEqual(receipt, "")

    def test_preexisting_read_only_mount_is_preserved(self):
        self.mounts.write_text("/dev/example /oos ext4 ro,noatime 0 0\n")
        calls, logs, receipt = self.run_helper()
        self.assertEqual(calls, ["sync"])
        self.assertEqual(logs, ["oos-shutdown-data: /oos already read-only"])
        self.assertEqual(receipt, "")

    def test_failed_sync_skips_mount_operations(self):
        calls, logs, _ = self.run_helper(code=1, SYNC_EXIT="1")
        self.assertEqual(calls, ["sync"])
        self.assertIn("sync failed; leaving /oos mounted", logs[0])

    def test_busy_writer_or_io_error_prevents_unmount(self):
        for error in ("Device or resource busy", "Input/output error"):
            with self.subTest(error=error):
                if self.closing.exists():
                    self.closing.rmdir()
                self.trace.write_text("")
                self.kmsg.write_text("")
                calls, logs, _ = self.run_helper(code=1, MOUNT_EXIT="1", MOUNT_ERROR=error)
                self.assertEqual(calls, ["sync", "mount -o remount,ro /oos"])
                self.assertIn(error, logs[0])
                self.assertIn("ext4 rw,noatime", self.mounts.read_text())
                self.assertTrue(self.closing.exists())

    def test_second_shutdown_does_not_take_over_write_admission(self):
        self.closing.mkdir()
        calls, _, receipt = self.run_helper(code=73)
        self.assertEqual(calls, [])
        self.assertEqual(receipt, "")
        self.assertTrue(self.closing.exists())

    def test_timeout_returns_the_worker_group_to_recovery(self):
        calls, logs, worker = self.run_helper(code=124, TIMEOUT_AT="sync")
        self.assertEqual(calls, ["sync"])
        self.assertEqual(logs, ["oos-shutdown-data: ui-drained timed out after 2s"])
        pid, mount = worker.splitlines()
        self.assertEqual(pid, "worker " + self.worker_pid.read_text().strip())
        self.assertEqual(mount, "mount /dev/example ext4 rw,noatime")
        self.assertTrue(self.closing.exists())

    def test_hung_unmount_returns_after_sealing_the_mount(self):
        calls, logs, receipt = self.run_helper(code=124, TIMEOUT_AT="umount")
        self.assertEqual(calls, ["sync", "mount -o remount,ro /oos", "umount /oos"])
        self.assertIn("ui-drained timed out after 2s", logs[-1])
        self.assertIn("mount /dev/example ext4 rw,noatime", receipt)
        self.assertIn("ext4 ro,noatime", self.mounts.read_text())
        self.assertTrue(self.closing.exists())

    def test_restore_waits_for_late_unmount_before_accepting_writable_data(self):
        calls, _, _ = self.run_helper("restore", "42", "/dev/example", "ext4", "rw,noatime", LATE_WORKER="1")
        self.assertEqual(calls, ["probe -42", "late umount completes", "probe -42",
                                 "mount -t ext4 -o rw,noatime /dev/example /oos"])
        self.assertIn("ext4 rw,noatime", self.mounts.read_text())

    def test_restore_rechecks_mount_after_a_successful_remount(self):
        self.mounts.write_text("/dev/example /oos ext4 ro,noatime 0 0\n")
        calls, _, _ = self.run_helper("restore", "", "/dev/example", "ext4", "rw,noatime")
        self.assertEqual(calls, ["mount -o remount,rw,noatime /oos", "mount -t ext4 -o rw,noatime /dev/example /oos"])
        self.assertIn("ext4 rw,noatime", self.mounts.read_text())

    def test_restore_demands_verified_writable_data_after_two_attempts(self):
        self.mounts.write_text("")
        calls, logs, _ = self.run_helper("restore", "", "/dev/example", "ext4", "rw,noatime",
                                         code=1, MOUNT_UPDATES_STATE="0")
        self.assertEqual(calls, ["mount -t ext4 -o rw,noatime /dev/example /oos"] * 2)
        self.assertEqual(logs, ["oos-shutdown-data: restore failed (worker status 1)"])
        self.assertTrue(self.closing.exists())

    def test_restore_reopens_admission_when_no_mount_was_changed(self):
        calls, logs, _ = self.run_helper("restore", "")
        self.assertEqual(calls, [])
        self.assertEqual(logs, [])
        self.assertFalse(self.closing.exists())
