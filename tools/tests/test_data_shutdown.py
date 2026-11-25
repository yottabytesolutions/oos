# SPDX-License-Identifier: GPL-2.0-or-later
"""Exercise bounded shutdown and recovery without devices or elapsed-time assertions."""
import os
from pathlib import Path
import re
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SHELL = shlex.split(os.environ.get("OOS_TEST_SHELL", "bash"))
BASE = "oos-base /oos tmpfs ro 0 0\n"
COVER = "oos-closed /oos tmpfs ro 0 0\n"
WRITABLE = "/dev/example /oos ext4 rw,noatime 0 0\n"
READ_ONLY = "/dev/example /oos ext4 ro,noatime 0 0\n"
USB = "/dev/example-usb /mnt/usb vfat rw 0 0\n"
SYNC = "sync -f /oos"
SEAL = "mount -t ext4 -o remount,ro /dev/example /oos"
REOPEN = "mount -t ext4 -o remount,rw,noatime /dev/example /oos"
MOUNT_DATA = "mount -t ext4 -o rw,noatime /dev/example /oos"
UNMOUNT = "umount /oos"
MOUNT_COVER = "mount -t tmpfs -o ro oos-closed /oos"
CLEANUP = [SYNC, SEAL, UNMOUNT, MOUNT_COVER]


class DataShutdown(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        self.trace = self.root / "trace"
        self.kmsg = self.root / "kmsg"
        self.marker = self.root / "off"
        self.closing = self.root / "closing"
        self.usb_mounting = self.root / "usb.mounting"
        self.mounts = self.root / "proc" / "mounts"
        self.trace.touch()
        self.kmsg.touch()
        self.mounts.parent.mkdir()
        self.mounts.write_text(BASE + WRITABLE)
        os.mkfifo(self.root / "blocked")
        os.mkfifo(self.root / "sleeping")
        os.mkfifo(self.root / "fallback")
        source = (ROOT / "rootfs/initramfs/bin/oos-shutdown-data").read_text()
        paths = {
            "/proc/": self.mounts.parent.as_posix() + "/",
            "/dev/kmsg": self.kmsg,
            "/tmp/oos-ui.off": self.marker,
            "/tmp/oos-data.closing": self.closing,
            "/tmp/oos-usb.mounting": self.usb_mounting,
            "/tmp/oos-shutdown.": self.root / "shutdown.",
        }
        source = source.replace(" > /dev/kmsg", " >> /dev/kmsg")
        source = re.sub("|".join(map(re.escape, paths)), lambda m: str(paths[m[0]]), source)
        self.worker_pid = self.root / "worker.pid"
        source = source.replace('setsid "$0"', '"$STUBS/setsid" "$0"')
        source = source.replace("/bin/sync", '"$STUBS/sync"')
        source = source.replace("/bin/umount", '"$STUBS/umount"')
        source = source.replace('    shift\n    case $1 in', '    printf "%s\\n" "$$" > "$WORKER_PID"\n    shift\n    case $1 in')
        stubs = ("killall", "pidof", "umount", "mount", "mktemp", "mkfifo")
        # The image's standalone ash can prefer applets to PATH, so point test functions at fixtures.
        shim = "".join(f'{name}() {{ "$STUBS/{name}" "$@"; }}\n' for name in stubs)
        shim += '''
sleep() {
    if [ -n "$SETUP_FAIL" ]; then
        read -r fallback < "$FALLBACK"
        printf 'deadline %s\\n' "$*" >> "$TRACE"
    fi
    if [ "$LATE_WORKER" = 1 ]; then
        printf 'late umount completes\\n' >> "$TRACE"
        grep -v 'ext4' "$MOUNTS" > "$MOUNTS.new"
        mv "$MOUNTS.new" "$MOUNTS"
    fi
}

if [ "$1" = --worker ]; then
    read() {
        if [ "$2" = source ] && [ -d "$USB_MOUNTING" ]; then
            # Publish the new mount after the helper has opened the old snapshot.
            { printf '%s\n' "$USB_MOUNT"; cat "$MOUNTS"; } > "$MOUNTS.new"
            mv "$MOUNTS.new" "$MOUNTS"
            rmdir "$USB_MOUNTING"
        fi
        if [ "$2" != -t ]; then command read "$@"; return; fi
        [ "$3" = 0.5 ] || exit 92
        if [ "$TIMEOUT_AT" = usb ]; then
            exec 6<"$SLEEPING"
            command read -r ready <&6 && [ "$ready" = blocked ] || exit 93
            return 1
        fi
        command read -r "$4"
    }
fi

kill() {
    if [ "$1" = -0 ] && [ "$2" = "-42" ]; then
        printf 'probe %s\n' "$2" >> "$TRACE"
        [ "$LATE_WORKER" = 1 ] && grep -q 'ext4' "$MOUNTS"
    else
        command kill "$@"
        if [ "$TIMEOUT_AT" = usb ] && [ "$1" = -TERM ] && [ "$2" = "$usb_worker" ]; then
            # EOF proves the USB umount child was killed before the test proceeds.
            if command read -r unexpected <&6; then exit 94; fi
        fi
        if [ "$CHECK_GROUP_KILL" = 1 ] && [ "$1" = -KILL ] && [ "$2" = "-$worker" ]; then
            # The sync child alone holds this FIFO open. EOF proves the group signal reached it.
            if read -r unexpected <&4; then exit 90; fi
        fi
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
        for name in ("mktemp", "mkfifo"):
            self.stub(name, f'[ "$SETUP_FAIL" != {name} ] || exit 1\nexec {shlex.quote(shutil.which(name))} "$@"\n')
        self.stub("blocked-sync", r'''import os
from pathlib import Path
Path(os.environ["SLEEPER_PID"]).write_text(f"{os.getpid()} {os.getpgrp()}\n")
with open(os.environ["SLEEPING"], "w") as ready:
    ready.write("blocked\n")
    ready.flush()
    with open(os.environ["BLOCKED"]) as blocked:
        blocked.read()
''', sys.executable)
        self.stub("sync", '''printf 'sync %s\n' "$*" >> "$TRACE"
if [ -n "$SETUP_FAIL" ]; then
    [ "$FALLBACK_BLOCKED" = 1 ] || exit 0
    printf 'started\n' > "$FALLBACK"
    exec cat "$BLOCKED"
fi
if [ "$CHECK_GROUP_KILL" = 1 ]; then
    exec "$STUBS/blocked-sync"
fi
[ -d "$CLOSING" ] || exit 2
if [ "$TIMEOUT_AT" = sync ]; then
    printf 'IO in progress\n' >&3
    exec cat "$BLOCKED"
fi
exit "$SYNC_EXIT"
''')
        self.stub("umount", '''printf "umount %s\n" "$*" >> "$TRACE"
if [ "$1" = /mnt/usb ]; then
    if [ "$TIMEOUT_AT" = usb ]; then
        exec 5>&-
        exec "$STUBS/blocked-sync"
    fi
    [ "$USB_EXIT" = 0 ] || exit "$USB_EXIT"
    sed '\\| /mnt/usb |d' "$MOUNTS" > "$MOUNTS.new"
    mv "$MOUNTS.new" "$MOUNTS"
    exit
fi
if [ "$TIMEOUT_AT" = umount ]; then
    printf 'IO in progress\n' >&3
    exec cat "$BLOCKED"
fi
[ "$UMOUNT_EXIT" = 0 ] || { echo "Device or resource busy" >&2; exit "$UMOUNT_EXIT"; }
sed '$d' "$MOUNTS" > "$MOUNTS.new"
mv "$MOUNTS.new" "$MOUNTS"
''')
        self.stub("mount", '''printf "mount %s\n" "$*" >> "$TRACE"
case "$*" in
    '-t ext4 -o remount,ro /dev/example /oos')
        [ "$MOUNT_EXIT" = 0 ] || { echo "$MOUNT_ERROR" >&2; exit "$MOUNT_EXIT"; }
        sed 's/ext4 rw/ext4 ro/' "$MOUNTS" > "$MOUNTS.new"
        mv "$MOUNTS.new" "$MOUNTS" ;;
    '-t ext4 -o remount,rw,noatime /dev/example /oos')
        if [ "${REMOUNT_DISAPPEARS:-0}" = 1 ]; then
            sed '$d' "$MOUNTS" > "$MOUNTS.new"
        else
            sed 's/ext4 ro/ext4 rw/' "$MOUNTS" > "$MOUNTS.new"
        fi
        mv "$MOUNTS.new" "$MOUNTS" ;;
    '-t ext4 -o rw,noatime /dev/example /oos')
        if [ "$MOUNT_UPDATES_STATE" = 1 ]; then
            echo '/dev/example /oos ext4 rw,noatime 0 0' >> "$MOUNTS"
        fi ;;
    '-t tmpfs -o ro oos-closed /oos')
        [ "$COVER_EXIT" = 0 ] || { echo "$MOUNT_ERROR" >&2; exit "$COVER_EXIT"; }
        echo 'oos-closed /oos tmpfs ro 0 0' >> "$MOUNTS" ;;
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
        if overrides.get("SETUP_FAIL") == "marker":
            self.marker.mkdir()
        env = {
            **os.environ,
            "STUBS": str(self.root),
            "TRACE": str(self.trace),
            "WORKER_PID": str(self.worker_pid),
            "MARKER": str(self.marker),
            "CLOSING": str(self.closing),
            "MOUNTS": str(self.mounts),
            "USB_MOUNTING": str(self.usb_mounting),
            "USB_MOUNT": USB.rstrip(),
            "BLOCKED": str(self.root / "blocked"),
            "FALLBACK": str(self.root / "fallback"),
            "SLEEPING": str(self.root / "sleeping"),
            "SLEEPER_PID": str(self.root / "sleeper.pid"),
            "CHECK_GROUP_KILL": "0",
            "SETUP_FAIL": "",
            "FALLBACK_BLOCKED": "0",
            "UMOUNT_EXIT": "0",
            "USB_EXIT": "0",
            "COVER_EXIT": "0",
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
        # Deliver simulated timeouts after the worker signals that it is blocked.
        driver = '''
read() {
    if [ "$2" != -t ]; then command read "$@"; return; fi
    [ "$1" = -r ] && [ "$3" = "$EXPECTED_BOUND" ] || return 2
    if [ "$CHECK_GROUP_KILL" = 1 ]; then
        exec 4<"$SLEEPING"
        command read -r ready <&4 && [ "$ready" = blocked ] || exit 91
        return 1
    fi
    command read -r "$4" || return
    [ "$result" != "IO in progress" ]
}
. "$0"
'''
        try:
            result = subprocess.run([*SHELL, "-c", driver, str(self.script), mode, *args],
                                    env=env, capture_output=True, text=True, timeout=30)
        except subprocess.TimeoutExpired:
            sleeper = self.root / "sleeper.pid"
            if sleeper.exists() and sleeper.read_text().strip():
                try:
                    os.kill(int(sleeper.read_text().split()[0]), signal.SIGKILL)
                except ProcessLookupError:
                    pass
            raise
        self.assertEqual(result.returncode, code, result.stderr)
        self.assertEqual(list(self.root.glob("shutdown.*")), [], "EXIT trap left a FIFO directory")
        return self.trace.read_text().splitlines(), self.kmsg.read_text().splitlines(), result.stdout

    def test_stops_ui_before_sync_and_unmount(self):
        calls, logs, _ = self.run_helper("stop-ui")
        self.assertTrue(self.marker.exists())
        self.assertEqual(calls, ["killall -TERM oos-ui", "pidof oos-ui", *CLEANUP])
        self.assertEqual(logs, [])

    def test_unresponsive_ui_is_killed_before_sync(self):
        calls, _, _ = self.run_helper("stop-ui", STUBBORN_UI="1")
        self.assertEqual(calls, ["killall -TERM oos-ui", *(["pidof oos-ui"] * 30),
                                 "killall -KILL oos-ui", "pidof oos-ui", *CLEANUP])

    def test_drained_ui_does_not_disable_the_supervisor(self):
        calls, logs, _ = self.run_helper()
        self.assertFalse(self.marker.exists())
        self.assertTrue(self.closing.exists())
        self.assertEqual(calls, CLEANUP)
        self.assertEqual(logs, [])

    def test_removable_media_unmounts_before_data_in_both_modes(self):
        for mode in ("ui-drained", "stop-ui"):
            with self.subTest(mode=mode):
                self.mounts.write_text(USB + BASE + WRITABLE)
                self.trace.write_text("")
                if self.closing.exists():
                    self.closing.rmdir()
                calls, logs, _ = self.run_helper(mode)
                preceding = ["killall -TERM oos-ui", "pidof oos-ui"] if mode == "stop-ui" else []
                self.assertEqual(calls, [*preceding, "umount /mnt/usb", *CLEANUP])
                self.assertEqual(self.mounts.read_text(), BASE + COVER)
                self.assertEqual(logs, [])

    def test_failed_usb_unmount_still_unmounts_data(self):
        self.mounts.write_text(USB + BASE + WRITABLE)
        calls, logs, _ = self.run_helper(USB_EXIT="1")
        self.assertEqual(calls, ["umount /mnt/usb", *CLEANUP])
        self.assertIn("could not unmount /mnt/usb", logs[0])

    def test_automount_finishing_during_the_mount_scan_is_unmounted(self):
        self.usb_mounting.mkdir()
        calls, logs, _ = self.run_helper()
        self.assertEqual(calls, ["umount /mnt/usb", *CLEANUP])
        self.assertEqual(logs, [])

    def test_usb_timeout_still_unmounts_data(self):
        self.mounts.write_text(USB + BASE + WRITABLE)
        calls, logs, _ = self.run_helper(TIMEOUT_AT="usb")
        self.assertEqual(calls, ["umount /mnt/usb", *CLEANUP])
        self.assertIn("/mnt/usb timed out after 0.5s", logs[0])
        self.assertEqual(self.mounts.read_text(), USB + BASE + COVER)

    def test_busy_unmount_stays_read_only(self):
        calls, logs, receipt = self.run_helper(code=1, UMOUNT_EXIT="1")
        self.assertEqual(calls, [SYNC, SEAL, UNMOUNT])
        self.assertIn("Device or resource busy", logs[0])
        self.assertIn("leaving /oos read-only", logs[0])
        self.assertEqual(self.mounts.read_text(), BASE + READ_ONLY)
        self.run_helper("restore", "", *receipt.split()[1:])
        self.assertEqual(self.mounts.read_text(), BASE + WRITABLE)
        self.assertFalse(self.closing.exists())

    def test_missing_data_mount_leaves_other_mounts_alone(self):
        self.mounts.write_text("/dev/example /oos-extra ext4 rw 0 0\n")
        calls, logs, receipt = self.run_helper()
        self.assertEqual((calls, logs, receipt), ([SYNC], ["oos-shutdown-data: /oos already unmounted"], ""))

    def test_preexisting_read_only_mount_is_preserved(self):
        self.mounts.write_text(BASE + READ_ONLY)
        calls, logs, receipt = self.run_helper()
        self.assertEqual((calls, logs, receipt), ([SYNC], ["oos-shutdown-data: /oos already read-only"], ""))

    def test_failed_sync_skips_mount_operations(self):
        calls, logs, _ = self.run_helper(code=1, SYNC_EXIT="1")
        self.assertEqual(calls, [SYNC])
        self.assertIn("sync of /oos failed", logs[0])

    def test_busy_writer_or_io_error_prevents_unmount(self):
        for error in ("Device or resource busy", "Input/output error"):
            with self.subTest(error=error):
                if self.closing.exists():
                    self.closing.rmdir()
                self.trace.write_text("")
                self.kmsg.write_text("")
                calls, logs, _ = self.run_helper(code=1, MOUNT_EXIT="1", MOUNT_ERROR=error)
                self.assertEqual(calls, [SYNC, SEAL])
                self.assertIn(error, logs[0])
                self.assertIn("ext4 rw,noatime", self.mounts.read_text())
                self.assertTrue(self.closing.exists())

    def test_second_shutdown_does_not_take_over_write_admission(self):
        self.closing.mkdir()
        calls, _, receipt = self.run_helper(code=73)
        self.assertEqual(calls, [SYNC])
        self.assertEqual(receipt, "")
        self.assertTrue(self.closing.exists())

    def test_init_shutdown_syncs_when_admission_was_already_closed(self):
        self.closing.mkdir()
        calls, _, receipt = self.run_helper("stop-ui", code=73)
        self.assertEqual(calls, ["killall -TERM oos-ui", "pidof oos-ui", SYNC])
        self.assertEqual(receipt, "")

    def test_sync_only_times_out_without_issuing_a_recovery_receipt(self):
        self.closing.mkdir()
        calls, logs, receipt = self.run_helper(code=73, TIMEOUT_AT="sync")
        self.assertEqual(calls, [SYNC])
        self.assertIn("sync-only timed out after 2s", logs[-1])
        self.assertEqual(receipt, "")

    def test_timeout_returns_the_worker_group_to_recovery(self):
        calls, logs, worker = self.run_helper(code=124, TIMEOUT_AT="sync")
        self.assertEqual(calls, [SYNC])
        self.assertEqual(logs, ["oos-shutdown-data: ui-drained timed out after 2s"])
        self.assertEqual(worker, f"worker {self.worker_pid.read_text().strip()}\n")
        self.assertTrue(self.closing.exists())
        self.run_helper("restore", "42")
        self.assertEqual(self.mounts.read_text(), BASE + WRITABLE)
        self.assertFalse(self.closing.exists())

    def test_timeout_kills_the_worker_group_including_its_sync_child(self):
        calls, _, receipt = self.run_helper(code=124, CHECK_GROUP_KILL="1")
        self.assertEqual(calls, [SYNC])
        sleeper, group = (self.root / "sleeper.pid").read_text().split()
        self.assertNotEqual(sleeper, group)
        self.assertEqual(group, self.worker_pid.read_text().strip())
        self.assertIn(f"worker {group}\n", receipt)

    def test_setup_failures_still_get_a_bounded_sync(self):
        for setup, mode, seconds in [("mktemp", "ui-drained", 2), ("mkfifo", "ui-drained", 2), ("marker", "stop-ui", 5)]:
            with self.subTest(setup=setup):
                self.trace.write_text("")
                calls, logs, receipt = self.run_helper(mode, code=73, SETUP_FAIL=setup, FALLBACK_BLOCKED="1")
                self.assertEqual((calls, receipt), ([SYNC, f"deadline {seconds}"], ""))
                self.assertIn(f"cannot start {mode}", logs[-1])
                self.assertFalse(self.closing.exists())

    def test_setup_failure_returns_when_sync_finishes_before_the_deadline(self):
        calls, logs, receipt = self.run_helper(code=73, SETUP_FAIL="mkfifo")
        self.assertEqual((calls, receipt), ([SYNC], ""))
        self.assertEqual(logs, ["oos-shutdown-data: cannot start ui-drained"])

    def test_hung_unmount_returns_after_sealing_the_mount(self):
        calls, _, receipt = self.run_helper(code=124, TIMEOUT_AT="umount")
        self.assertEqual(calls, [SYNC, SEAL, UNMOUNT])
        self.assertIn("mount /dev/example ext4 rw,noatime", receipt)
        self.assertIn("ext4 ro,noatime", self.mounts.read_text())
        self.assertTrue(self.closing.exists())

    def test_restore_waits_for_late_unmount_before_accepting_writable_data(self):
        calls, _, _ = self.run_helper("restore", "42", "/dev/example", "ext4", "rw,noatime", LATE_WORKER="1")
        self.assertEqual(calls, ["probe -42", "late umount completes", "probe -42", MOUNT_DATA])
        self.assertEqual(self.mounts.read_text(), BASE + WRITABLE)

    def test_restore_rechecks_mount_after_a_successful_remount(self):
        self.mounts.write_text(BASE + READ_ONLY)
        calls, _, _ = self.run_helper("restore", "", "/dev/example", "ext4", "rw,noatime", REMOUNT_DISAPPEARS="1")
        self.assertEqual(calls, [REOPEN, MOUNT_DATA])
        self.assertIn("ext4 rw,noatime", self.mounts.read_text())

    def test_restore_demands_verified_writable_data_after_two_attempts(self):
        self.mounts.write_text(BASE + COVER)
        calls, logs, _ = self.run_helper("restore", "", "/dev/example", "ext4", "rw,noatime",
                                         code=1, MOUNT_UPDATES_STATE="0")
        self.assertEqual(calls, [UNMOUNT, MOUNT_DATA, MOUNT_DATA])
        self.assertEqual(logs, ["oos-shutdown-data: restore failed (worker status 1)"])
        self.assertTrue(self.closing.exists())
        self.assertEqual(self.mounts.read_text(), BASE)

    def test_cover_stays_in_place_if_restore_cannot_remove_it(self):
        self.mounts.write_text(BASE + COVER)
        for code, settings in [(1, {"UMOUNT_EXIT": "1"}), (124, {"TIMEOUT_AT": "umount"})]:
            self.trace.write_text("")
            calls, _, receipt = self.run_helper("restore", "", "/dev/example", "ext4", "rw,noatime", code=code,
                                                **settings)
            self.assertEqual((calls, receipt), ([UNMOUNT], ""))
            self.assertEqual(self.mounts.read_text(), BASE + COVER)
            self.assertTrue(self.closing.exists())

    def test_base_protects_unmounted_area_when_cover_cannot_mount(self):
        calls, logs, receipt = self.run_helper(COVER_EXIT="1")
        self.assertEqual(calls, CLEANUP)
        self.assertEqual(logs, ["oos-shutdown-data: could not mount /oos cover: Input/output error"])
        self.assertEqual(self.mounts.read_text(), BASE)
        self.trace.write_text("")
        calls, _, _ = self.run_helper("restore", "", *receipt.split()[1:])
        self.assertEqual(calls, [MOUNT_DATA])
        self.assertEqual(self.mounts.read_text(), BASE + WRITABLE)
        self.assertFalse(self.closing.exists())

    def test_returned_reboot_restores_data_and_second_shutdown_can_close_it_again(self):
        for _ in range(2):
            self.trace.write_text("")
            calls, _, receipt = self.run_helper()
            self.assertEqual(calls, CLEANUP)
            self.assertEqual(self.mounts.read_text(), BASE + COVER)
            self.trace.write_text("")
            calls, _, _ = self.run_helper("restore", "", *receipt.split()[1:])
            self.assertEqual(calls, [UNMOUNT, MOUNT_DATA])
            self.assertEqual(self.mounts.read_text(), BASE + WRITABLE)
            self.assertFalse(self.closing.exists())

    def test_restore_after_failed_seal_keeps_the_original_writable_mount(self):
        _, _, receipt = self.run_helper(code=1, MOUNT_EXIT="1")
        self.trace.write_text("")
        calls, _, _ = self.run_helper("restore", "", *receipt.split()[1:])
        self.assertEqual(calls, [])
        self.assertEqual(self.mounts.read_text(), BASE + WRITABLE)
        self.assertFalse(self.closing.exists())
