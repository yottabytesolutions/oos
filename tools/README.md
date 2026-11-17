# OOS tools

| Tool | Touches the scope? | Purpose |
|---|---|---|
| `release-sources.sh list / check TAG` | no | the source files a release carries next to its image, from `rootfs/deps.lock` and `kernel/base.sha256`; `check` compares a release on GitHub with the list ([THIRD-PARTY-LICENSES.md](../THIRD-PARTY-LICENSES.md)) |
| `sdcard-clone.sh list / read / write / verify / fit` | card in a reader on macOS or Linux | clone the boot SD card to an image and onto the second card; refuses disks that are not SD or USB media, virtual disks and disks of the running system; `write` asks you to type the disk id |
| `sdcard-layout.py IMAGE [--extract DIR] [--compare DIR]` | no (reads an image or `/dev/rdiskN`) | IDB, vendor storage (MAC), PARM partition table, partition contents |
| `scope-lease {acquire,check,renew,release,status,break}` | SSH lease metadata only | exclusive access across machines and agent vendors |
| `device-test.py [--host HOST]` | SCPI, HTTP and SSH lease metadata | acceptance test of a running OOS with nobody at the scope; `--sim` runs it against `oos-ui --serve` on this computer |
| `scpi_validate.py --web-port 18080` | local `oos-ui` only | uploads analytic waveforms into R1-R3 and validates numeric SCPI, thresholds, source isolation, stopped panning and trigger/run/stop state; writes evidence to `local/scpi-validation.json` |
| `device-deploy.sh [--host HOST] [--restore]` | programs in `/tmp`, lease metadata in `/oos` | **owner-only**, requires an install lease; runs the oos-ui of this tree on a running OOS |
| `ssh-keys.py --data-dir DIR add / list / remove` | offline mounted data directory only | manage Ed25519 public keys without a network connection; [SSH setup](../docs/ssh.md) |
| `device-survey.sh [--host HOST] [--keys]` | read-only over adb | inventory of the running stock system: USB/HID, input, GPIO owners, pinmux, ttyS0, PCIe, partitions |
| `platform-probe.sh [--host HOST]` | read-only over adb, removes its files again | description of another Rigol model (MHO900, DHO1000, ...) for a port, ready to share: device tree, buses, display, partition table, `/rigol` by name; serial number and MAC addresses replaced ([docs/porting.md](../docs/porting.md)) |

`device-test.py` also has the tiers `la` (`--pico <port>`: D0-D2 on GP2-GP4 of a Pico with the openSciLab firmware) and `rf`
(`--rf <port>`: an HT008 on `--rf-ch`, CH2 by default). `--busy 3` names other channels with a cable, which are no open inputs.

`../boot/oos-bootimg.py` builds the `recovery` boot image for the stock loader. `../kernel/build-base.sh` rebuilds
the GPL kernel in Docker (everything in `../kernel/`).

## Scope lease

Get the owner's release of the unit for your task first. Every device tool requires
`--host HOST` or `OOS_SCOPE`. There is no default device address. Positional device
addresses are no longer accepted. Simulator addresses are local test fixtures.
Use a bare host such as `scope.example` or `192.0.2.1`, without a user or port.
The lease tools need SSH enabled on the unit with a trusted key. `remote.ssh` is
off by default. SSH uses root with existing trusted host keys and credentials.
The tools do not enroll keys.

`platform-probe.sh` is for other people's stock units. It uses adb without an SSH
lease. `device-survey.sh` also uses stock adb without an SSH lease. Stock Android
has no SSH daemon or `/oos` lease directory. Survey prints a reminder before adb
access: our shared unit requires the owner's release for that task. This release
is still required for all access to our shared unit, including stock adb tools.
`OOS_ADB_PORT` sets the adb port. Its default is 55555.

Choose a unique holder for your session. Keep the holder, task and token in the
environment for every OOS device tool. All vendors use the same command and device
directory.

```sh
export OOS_SCOPE=scope.example
export OOS_LEASE_HOLDER=agent-a-session
export OOS_LEASE_TASK=0021
tools/scope-lease acquire --cap test --wiring 'CH1 to reference; CH2 open'
# Set this to info.token from the acquire JSON output. Keep it private.
export OOS_LEASE_TOKEN='<token from acquire>'
tools/scope-lease status
python3 tools/device-test.py --tier a0
tools/scope-lease renew
tools/scope-lease release
```

Capabilities are ordered: `observe < test < install < reboot`. A higher capability
allows the lower ones. Device test requires `test`. Deploy is owner-only. It
checks `install` before any upload or shell command. Agents install only signed
packages through `oos-update` or the web update page.
An install or reboot lease requires `acquire --cap install --go 0021` or
`acquire --cap reboot --go 0021` for task `0021`. The owner must give that go for
that task. An agent never issues a go for its own task. The coordinator relays the
owner's go. The tool records the go. It cannot verify that a human gave it.
Testing permission never implies that go. To change capability, release and
acquire again with the required authorization.

`check --cap test` and `status --cap test` check the caller's holder, task, token
and minimum capability. Check, renew and release require `OOS_LEASE_TOKEN` or
`--token`. Names alone do not grant access. Acquire prints the token in its JSON
output. All other commands omit it. Plain `status` needs no caller identity or
token. `--holder` and `--task` override the environment for lease commands.
The deploy shell tool performs an initial check. Renew every two minutes during
its use, including long deploys.

The lease is `/oos/lease`. Atomic `mkdir` over SSH acquires it. Its `info` records
holder, task, capability, SHA-256 of the caller's machine name, build,
start wall time, heartbeat uptime seconds, boot ID, task go and a unique lease
token. Optional `--wiring` records up to 512 characters of free text on one line. It is JSON quoted in the
record and shell quoted during transport. Existing records without wiring remain
valid. Keep wiring text free of private data.
The build comes from `/etc/oos-build`, the source used by the update tools.
No raw machine name or SCPI serial number is recorded. The boot ID comes from
`/proc/sys/kernel/random/boot_id`. Heartbeats and age checks use whole seconds
from `/proc/uptime` inside the metadata guard. Wall time from `date +%s` is
informational, for the start time and logs. Clock resets and NTP jumps do not
change lease age. The heartbeat worker uses monotonic elapsed time to
stop before the last device-confirmed expiry.

Installs and reboots under a lease can change the boot ID. After the unit returns,
`status` shows the persistent lease as quarantined. Check, renew or release reports
`unit rebooted under lease` unless the holder supplies the matching task and token
and the lease has install or reboot capability. That authorized holder can rebind
through check, renew or release. Rebinding logs the old and new boot IDs before
updating the record. It resets the heartbeat to the new boot's uptime. It preserves
the token, go and capability. Other callers need an owner-authorized break.
Old records without a boot ID are incomplete and need inspection and a break.
Rebinding does not authorize an install or reboot. Keep the owner's go for that
task. After reboot, wait for `OOSK` re-arm before the next reboot. Never reboot
twice within 60 seconds. This lifecycle is covered by offline tests. It has not
been verified on the unit.

A short `/oos/lease.guard` directory serializes metadata operations. Contention
retries with backoff for up to 3.1 seconds. Cleanup traps cover normal exit, PIPE,
HUP, INT and TERM. Updates compare the old record before changing it. A stale
renew or release cannot change a replacement lease. Unexpected metadata or
persistent guard contention refuses access. No guard is removed automatically.

A heartbeat older than ten minutes quarantines the lease. Acquire, renew and
release refuse it. `status` shows the holder, capability, build and timestamps.
An incomplete record also quarantines the unit. Never auto-transfer a lease.
After inspection and with the owner's authorization, clear it explicitly:

```sh
tools/scope-lease break --reason 'Owner inspected the unit after interrupted testing'
```

A stranded guard can survive an uncatchable signal or lost process. After owner
inspection, break only that guard with:

```sh
tools/scope-lease break --guard --reason 'Owner inspected the stranded metadata guard'
```

Guard break refuses a guard aged 60 seconds or less by the device clock. It logs
the reason, caller, task, device time and guard timestamp before removing the
empty directory. It leaves the lease intact. Log failure preserves the guard.
Never break another holder's lease or guard without the owner. Lease break logs
the reason, caller, task, device time and full old info before removal. Both forms
append to `/oos/lease.log`. They do not repair or reboot the unit. Agents stop and
report unexpected unit state. They never attempt automatic recovery.

Python tools can keep an existing lease alive for the duration of their work:

```python
import scope_lease

with scope_lease.held(host, cap="test") as lease:
    lease.check()  # Check before each device IO operation.
    run_device_operation()
```

The context checks `OOS_LEASE_HOLDER`, `OOS_LEASE_TASK` and `OOS_LEASE_TOKEN`.
It renews on entry and every two minutes in a background thread. Failed background
renewals retry with backoff, capped at five seconds. They report failure at the
last confirmed expiry minus a 75 second margin. This leaves time for both bounded
SSH exchanges in a renewal to finish. Each IO check also enforces that deadline.
Context exit stops and joins the worker. It makes no further executor calls after
exit.
The context keeps the lease for explicit release. Device test stops IO when the
lease is lost. `device-test.py --sim` uses no device lease.

Hardware behavior is not yet verified on the unit. Run the offline checks with:

```sh
python3 -I -m unittest discover -s tools/tests
python3 -I -m py_compile tools/scope_lease.py tools/scope-lease tools/device-test.py tools/tests/test_scope_lease*.py
sh -n tools/device-deploy.sh tools/device-survey.sh tools/platform-probe.sh
```

The tests inject a fake SSH executor. It runs the actual shell protocol against a
private temporary directory, including concurrent atomic acquire. No host is contacted.

A rebind has no age bound: a token holder can resume a lease after the unit was off for days. Break the lease instead if the unit was used by someone else in between.
