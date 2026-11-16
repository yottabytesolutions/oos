#!/usr/bin/env python3
# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
"""Device-backed scope leases. No operation selects a default device."""

from __future__ import annotations

from contextlib import contextmanager
from dataclasses import asdict, dataclass
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import shlex
import socket
import subprocess
import threading
import time
from typing import Callable, Iterator
import uuid

CAPABILITIES = ("observe", "test", "install", "reboot")
EXPIRY = 600
RENEW_INTERVAL = 120
RENEW_MARGIN = 75
GUARD_MAX_AGE = 60
WIRING_LIMIT = 512
LEASE = Path("/oos/lease")
GUARD = Path("/oos/lease.guard")
LOG = Path("/oos/lease.log")
Executor = Callable[[str, str], str]


def lease_token(value: str | None = None) -> str:
    value = value if value is not None else os.environ.get("OOS_LEASE_TOKEN", "")
    if not re.fullmatch(r"[a-f0-9]{32}", value):
        raise LeaseError("set OOS_LEASE_TOKEN or --token to the token printed by acquire")
    return value


def wiring_value(value: str) -> str:
    if (not isinstance(value, str) or len(value) > WIRING_LIMIT
            or any(ord(char) < 32 or ord(char) == 127 for char in value)):
        raise LeaseError(f"wiring must be a single line of at most {WIRING_LIMIT} characters")
    return value


class LeaseError(RuntimeError):
    """Access refused. Stop using the scope."""


def host_value(host: str | None = None) -> str:
    value = host if host is not None else os.environ.get("OOS_SCOPE", "")
    if not value:
        raise LeaseError("specify --host or set OOS_SCOPE; no default scope")
    if not re.fullmatch(r"[A-Za-z0-9_.:\[\]-]+", value) or value.startswith("-"):
        raise LeaseError("invalid scope host; use a bare host without user or port")
    return value


def identity(holder: str | None = None, task: str | None = None) -> tuple[str, str]:
    holder = holder if holder is not None else os.environ.get("OOS_LEASE_HOLDER", "")
    task = task if task is not None else os.environ.get("OOS_LEASE_TASK", "")
    for name, value in (("holder", holder), ("task", task)):
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,127}", value):
            raise LeaseError(f"set OOS_LEASE_{name.upper()} or --{name} to a plain identifier")
    return holder, task


def capability(value: str) -> int:
    try:
        return CAPABILITIES.index(value)
    except ValueError as exc:
        raise LeaseError(f"unknown capability: {value}") from exc


def ssh(host: str, script: str) -> str:
    """Use configured SSH trust and credentials. Never change host keys."""
    try:
        result = subprocess.run(
            ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
             "-o", "ConnectTimeout=10", f"root@{host}", "sh -s"],
            input=script, text=True, capture_output=True, timeout=30, check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise LeaseError("lease SSH failed or timed out; stop scope access") from exc
    if result.returncode:
        raise LeaseError(result.stderr.strip() or "lease SSH failed; stop scope access")
    return result.stdout


@dataclass(frozen=True)
class Info:
    holder: str
    task: str
    capability: str
    machine_hash: str
    build_id: str
    started: int
    heartbeat: int
    go: str
    token: str
    boot_id: str
    wiring: str = ""

    def encode(self) -> str:
        fields = asdict(self)
        fields["wiring"] = json.dumps(wiring_value(self.wiring))
        return "".join(f"{key}={value}\n" for key, value in fields.items())

    @classmethod
    def decode(cls, raw: str) -> Info:
        try:
            pairs = [line.split("=", 1) for line in raw.splitlines()]
            fields = dict(pairs)
            if len(fields) != len(pairs):
                raise ValueError("duplicate fields")
            fields["started"] = int(fields["started"])
            fields["heartbeat"] = int(fields["heartbeat"])
            fields["wiring"] = json.loads(fields.get("wiring", '\"\"'))
            info = cls(**fields)
            wiring_value(info.wiring)
            identity(info.holder, info.task)
            capability(info.capability)
            lease_token(info.token)
            if not info.build_id or not info.boot_id or info.heartbeat < 0:
                raise ValueError("incomplete record")
            if capability(info.capability) >= capability("install") and info.go != info.task:
                raise ValueError("missing task go")
            return info
        except (KeyError, TypeError, ValueError, LeaseError) as exc:
            raise LeaseError("quarantined lease: invalid or incomplete info; owner must inspect and break") from exc

    def expired(self, now: int) -> bool:
        return now - self.heartbeat > EXPIRY


class Client:
    def __init__(self, host: str | None = None, *, executor: Executor = ssh,
                 clock: Callable[[], float] = time.monotonic):
        self.host = host_value(host)
        self.executor = executor
        self.clock = clock
        self.device_now = 0
        self.device_boot_id = ""
        self.observed_at = clock()

    def _run(self, body: str) -> str:
        # Bound contention to 3.1 seconds. Never remove a busy guard automatically.
        script = f"""set -eu
umask 077
fail() {{ echo "$*" >&2; exit 1; }}
interrupted=0
trap 'interrupted=1' PIPE HUP INT TERM
for delay in 0 0.1 0.2 0.4 0.8 1.6; do
    [ "$delay" = 0 ] || sleep "$delay"
    if mkdir {GUARD} 2>/dev/null; then
        # Defer signals until mkdir's result is known and cleanup is armed.
        trap 'rmdir {GUARD}' EXIT
        trap 'exit 1' PIPE HUP INT TERM
        [ "$interrupted" = 0 ] || exit 1
        break
    fi
    [ "$interrupted" = 0 ] || fail 'lease operation interrupted'
    [ "$delay" != 1.6 ] || fail 'lease operation busy; owner must inspect the guard'
done
wall=$(date +%s)
read -r uptime rest < /proc/uptime
now=${{uptime%%.*}}
boot_id=$(cat /proc/sys/kernel/random/boot_id)
printf '%s\\n' "$now $boot_id"
[ ! -L {LEASE} ] || fail 'quarantined lease: unexpected symlink'
{body}
"""
        observed_at = self.clock()
        timestamp, separator, result = self.executor(self.host, script).partition("\n")
        parts = timestamp.split()
        if not separator or len(parts) != 2 or not parts[0].isdecimal():
            raise LeaseError("invalid device clock response; stop scope access")
        self.device_now, self.observed_at = int(parts[0]), observed_at
        self.device_boot_id = parts[1]
        return result

    @staticmethod
    def _live_script() -> str:
        return f"""[ -f {LEASE}/info ] || fail 'quarantined lease: incomplete info'
[ "$(sed -n 's/^boot_id=//p' {LEASE}/info)" = "$boot_id" ] || fail 'unit rebooted under lease; owner must inspect and break'
heartbeat=$(sed -n 's/^heartbeat=//p' {LEASE}/info)
case "$heartbeat" in ''|*[!0-9]*) fail 'quarantined lease: invalid heartbeat';; esac
[ "$now" -ge "$heartbeat" ] || fail 'device uptime moved backwards; stop scope access'
[ "$((now - heartbeat))" -le {EXPIRY} ] || fail 'expired lease quarantined; owner must inspect and break'"""

    def _raw(self, live: bool = False, allow_missing: bool = False) -> str | None:
        result = self._run(f"""if [ -d {LEASE} ]; then
    printf 'present\\n'
    [ ! -L {LEASE}/info ] || fail 'quarantined lease: unexpected info symlink'
    {':' if allow_missing else f"[ -f {LEASE}/info ] || fail 'quarantined lease: incomplete info'"}
    {self._live_script() if live else ':'}
    if [ -f {LEASE}/info ]; then cat {LEASE}/info; fi
elif [ -e {LEASE} ]; then
    fail 'quarantined lease: unexpected file'
fi""")
        return result[len("present\n"):] if result.startswith("present\n") else None

    def status(self) -> Info | None:
        raw = self._raw()
        return None if raw is None else Info.decode(raw)

    def acquire(self, holder: str, task: str, cap: str = "observe", go: str = "",
                wiring: str = "") -> Info:
        holder, task = identity(holder, task)
        wiring_value(wiring)
        if capability(cap) >= capability("install") and go != task:
            raise LeaseError("install and reboot require --go matching the lease task")
        if go and go != task:
            raise LeaseError("--go must match the lease task")
        info = Info(holder, task, cap, hashlib.sha256(socket.gethostname().encode()).hexdigest(),
                    "", 0, 0, go, uuid.uuid4().hex, "", wiring)
        # Failed initialization leaves the directory quarantined.
        fields = info.encode().replace("build_id=\n", "").replace("started=0\nheartbeat=0\n", "").replace("boot_id=\n", "")
        body = f"""mkdir {LEASE} 2>/dev/null || fail 'lease exists, possibly expired and quarantined; run status'
build=$(cat /etc/oos-build) || fail 'cannot read build; lease quarantined'
[ -n "$build" ] || fail 'empty build; lease quarantined'
printf '%s' {shlex.quote(fields)} > {LEASE}/info.new
printf 'build_id=%s\\nstarted=%s\\nheartbeat=%s\\nboot_id=%s\\n' "$build" "$wall" "$now" "$boot_id" >> {LEASE}/info.new
mv {LEASE}/info.new {LEASE}/info
cat {LEASE}/info"""
        return Info.decode(self._run(body))

    def _swap(self, raw: str, body: str, live: bool = False) -> str:
        return self._run(f"""[ -d {LEASE} ] && [ ! -L {LEASE}/info ] || fail 'lease changed; stop scope access'
[ "$(cat {LEASE}/info 2>/dev/null || true)" = {shlex.quote(raw.rstrip(chr(10)))} ] || fail 'lease changed; stop scope access'
{self._live_script() if live else ':'}
{body}""")

    @staticmethod
    def _authorize(info: Info, holder: str, task: str, cap: str, token: str) -> None:
        if (info.holder, info.task) != identity(holder, task):
            raise LeaseError(f"lease belongs to {info.holder}, task {info.task}")
        if not hmac.compare_digest(info.token, token):
            raise LeaseError("lease token does not match; stop scope access")
        if capability(info.capability) < capability(cap):
            raise LeaseError(f"lease capability {info.capability} does not permit {cap}")

    def _checked(self, holder: str, task: str, cap: str, token: str | None) -> tuple[str, Info]:
        identity(holder, task)
        capability(cap)
        token = lease_token(token)
        raw = self._raw()
        if raw is None:
            raise LeaseError("no lease; acquire one before scope access")
        info = Info.decode(raw)
        self._authorize(info, holder, task, cap, token)
        if info.boot_id != self.device_boot_id:
            if capability(info.capability) < capability("install"):
                raise LeaseError("unit rebooted under lease; owner must inspect and break")
            fields = info.encode().replace(f"boot_id={info.boot_id}\n", "").replace(f"heartbeat={info.heartbeat}\n", "")
            entry = json.dumps(dict(event="unit rebooted under lease", old_boot_id=info.boot_id,
                                    holder=holder, task=task))
            raw = self._swap(raw,
                f"printf '%s,\"boot_id\":\"%s\",\"at\":%s}}\\n' {shlex.quote(entry[:-1])} \"$boot_id\" \"$wall\" >> {LOG}\n"
                f"printf '%s' {shlex.quote(fields)} > {LEASE}/info.new\n"
                f"printf 'heartbeat=%s\\nboot_id=%s\\n' \"$now\" \"$boot_id\" >> {LEASE}/info.new\n"
                f"mv {LEASE}/info.new {LEASE}/info\ncat {LEASE}/info")
        else:
            raw = self._swap(raw, f"cat {LEASE}/info", live=True)
        return raw, Info.decode(raw)

    def check(self, holder: str, task: str, cap: str = "observe", token: str | None = None) -> Info:
        return self._checked(holder, task, cap, token)[1]

    def renew(self, holder: str, task: str, cap: str = "observe", token: str | None = None) -> Info:
        raw, info = self._checked(holder, task, cap, token)
        fields = info.encode().replace(f"heartbeat={info.heartbeat}\n", "")
        return Info.decode(self._swap(raw,
            f"printf '%s' {shlex.quote(fields)} > {LEASE}/info.new\n"
            f"printf 'heartbeat=%s\\n' \"$now\" >> {LEASE}/info.new\n"
            f"mv {LEASE}/info.new {LEASE}/info\ncat {LEASE}/info", live=True))

    def release(self, holder: str, task: str, token: str | None = None) -> None:
        raw, _ = self._checked(holder, task, "observe", token)
        self._swap(raw, f"rm {LEASE}/info\nrmdir {LEASE}", live=True)

    def break_lease(self, reason: str, holder: str, task: str, *, guard: bool = False) -> None:
        holder, task = identity(holder, task)
        if not reason.strip():
            raise LeaseError("break requires --reason with nonempty text")
        entry = json.dumps(dict(reason=reason, holder=holder, task=task))
        if guard:
            # Explicit owner action only. rmdir refuses unexpected guard contents.
            script = f"""set -eu
umask 077
[ -d {GUARD} ] && [ ! -L {GUARD} ] || {{ echo 'no plain guard to break' >&2; exit 1; }}
stamp=$(stat -c %Y {GUARD})
now=$(date +%s)
[ "$((now - stamp))" -gt {GUARD_MAX_AGE} ] || {{ echo 'guard is not older than 60 s' >&2; exit 1; }}
printf '%s,"at":%s,"guard_mtime":%s,"guard":true}}\\n' {shlex.quote(entry[:-1])} "$now" "$stamp" >> {LOG}
[ "$(stat -c %Y {GUARD})" = "$stamp" ] || exit 1
rmdir {GUARD}
"""
            self.executor(self.host, script)
            return
        raw = self._raw(allow_missing=True)
        if raw is None:
            raise LeaseError("no lease to break")
        entry = json.dumps(dict(reason=reason, old_info=raw, holder=holder, task=task))
        self._swap(raw, f"printf '%s,\"at\":%s}}\\n' {shlex.quote(entry[:-1])} \"$wall\" >> {LOG}\n"
                        f"rm -f {LEASE}/info {LEASE}/info.new\nrmdir {LEASE}")


@dataclass
class HeldLease:
    client: Client
    info: Info
    error: LeaseError | None = None
    deadline: float = 0

    def __post_init__(self) -> None:
        if self.info is not None:
            self.deadline = (self.client.observed_at + self.info.heartbeat + EXPIRY
                             - self.client.device_now - RENEW_MARGIN)

    def check(self) -> None:
        if self.error is not None:
            raise self.error
        if self.client.clock() >= self.deadline:
            raise LeaseError("lease renewal failed before expiry margin; stop scope access")

    def _heartbeat(self, stop: threading.Event, interval: float) -> None:
        delay, backoff = interval, 0.2
        while not stop.wait(max(0, min(delay, self.deadline - self.client.clock()))):
            try:
                self.check()
                renewed = self.client.renew(self.info.holder, self.info.task, self.info.capability,
                                            token=self.info.token)
                self.check()
                self.info = renewed
                self.__post_init__()
                delay, backoff = interval, 0.2
            except Exception as exc:
                if self.client.clock() >= self.deadline:
                    self.error = LeaseError(f"lease renewal failed; stop scope access: {exc}")
                    return
                delay, backoff = backoff, min(backoff * 2, 5)


@contextmanager
def held(host: str | None = None, cap: str = "test", *, executor: Executor = ssh,
         clock: Callable[[], float] = time.monotonic,
         interval: float = RENEW_INTERVAL) -> Iterator[HeldLease]:
    """Check an existing lease and keep it alive. The caller releases it explicitly.

    Call lease.check() before IO so a failed heartbeat stops subsequent access.
    """
    holder, task = identity()
    client = Client(host, executor=executor, clock=clock)
    lease = HeldLease(client, client.renew(holder, task, cap))
    stop = threading.Event()
    worker = threading.Thread(target=lease._heartbeat, args=(stop, interval), daemon=True)
    worker.start()
    try:
        yield lease
    finally:
        stop.set()
        worker.join()
    lease.check()


def main(argv: list[str] | None = None, *, executor: Executor = ssh,
         clock: Callable[[], float] = time.monotonic) -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("acquire", "check", "renew", "release", "status", "break"))
    parser.add_argument("--host", help="scope host, otherwise OOS_SCOPE; no default")
    parser.add_argument("--holder", help="otherwise OOS_LEASE_HOLDER")
    parser.add_argument("--task", help="otherwise OOS_LEASE_TASK")
    parser.add_argument("--cap", choices=CAPABILITIES, help="acquire or require this capability")
    parser.add_argument("--go", default="", help="owner's go for this task, recorded at acquire")
    parser.add_argument("--reason", default="", help="why the owner authorized breaking the lease")
    parser.add_argument("--guard", action="store_true", help="break a guard older than 60 seconds")
    parser.add_argument("--token", help="otherwise OOS_LEASE_TOKEN; required to check, renew or release")
    parser.add_argument("--wiring", default="", help="optional wiring description, at most 512 characters")
    args = parser.parse_args(argv)
    if args.guard and args.command != "break":
        parser.error("--guard requires break")
    try:
        client = Client(args.host, executor=executor, clock=clock)
        if args.command == "status" and not args.cap:
            info = client.status()
        else:
            holder, task = identity(args.holder, args.task)
            cap = args.cap or "observe"
            if args.command == "acquire":
                info = client.acquire(holder, task, cap, args.go, args.wiring)
            elif args.command == "renew":
                info = client.renew(holder, task, cap, args.token)
            elif args.command == "release":
                client.release(holder, task, args.token)
                print("lease released")
                return 0
            elif args.command == "break":
                client.break_lease(args.reason, holder, task, guard=args.guard)
                print("guard broken" if args.guard else "lease broken")
                return 0
            else:
                info = client.check(holder, task, cap, args.token)
        state = "free" if info is None else ("quarantined" if info.boot_id != client.device_boot_id or info.expired(client.device_now) else "held")
        record = asdict(info) if info else None
        if record is not None and args.command != "acquire":
            record.pop("token")
        print(json.dumps(dict(state=state, info=record), sort_keys=True))
        return 0
    except LeaseError as exc:
        import sys

        print(f"scope-lease: {exc}", file=sys.stderr)
        return 1
