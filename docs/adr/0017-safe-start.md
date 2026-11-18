# Safe start breaks restart loops

- Number: 0017
- Category: system
- State: in force
- Date: 2026-11-18

## Context

The roll OOM of 2026-11-14 exposed a restart loop: rcS restarted oos-ui, which restored the same failing setup. Release
builds abort on panic, and the setup survives power cycles, so repeated restarts alone cannot recover. The incident and
restart contract are recorded in [boot.md](../boot.md#restart-after-a-panic-or-a-hang).

## Decision

Exit 0 is clean; other exits and signals are abnormal. After 3 abnormal exits in one hour, or one exit 75 (memory), the
supervisor starts --safe with defaults and retains the offending setup as oos-ui.setup.bad. A setup is promoted for
automatic restore only after 60 seconds without abnormal exit. Setup, marker and ledger writes use temporary file,
fsync, rename and directory fsync. Storage failure disables persistence for that process and shows it; it never stops
the UI.

After stock selection is confirmed, kernel panic restart is 10 seconds; it is disabled before that safety point. The
next boot archives pstore to /oos/crash within 1 MiB and counts crash evidence once. A previous-boot running marker
counts only with crash evidence; without evidence it is a power cut. Safe start is off during an update trial; the
update watcher owns that decision.

Check: rootfs supervisor/crash tests and HAL startup tests; restart-loop stress round on the unit.

## Alternatives

Always restore the latest setup after a restart: rejected because the roll OOM repeated indefinitely. Count every
unclean running marker as a crash: rejected because a power cut need not indicate a faulty setup.

## Consequences

Watchdog reset reason on RK3399 is not yet verified on hardware. Ledger locking, timeouts and reset-evidence limits live
in docs/boot.md.
