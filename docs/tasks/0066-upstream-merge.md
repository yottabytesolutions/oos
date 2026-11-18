---
id: 66
title: Merge upstream (PR 7, PR 8, main) into overhaul
kind: chore
status: active
release: none
owner: claude
depends: []
---

# Merge upstream (PR 7, PR 8, main) into overhaul

## Goal

Bring upstream's work since our scrubbed PR 7 into `overhaul`: the PR 7 merge, PR 8 and upstream `main`. Keep our
acquisition correctness work and both draw paths. Review tier: Unit (acquisition and start-up code change).

## Plan

Reserved paths: whole tree on branch `integ-20-upstream` (integration merge), this task.

1. Bridge: `git merge -s ours` of upstream's PR 7 head, the scrubbed copy of our private branch at a commit that is
   already an ancestor of `overhaul`. The tree does not change; it gives upstream and `overhaul` a merge base.
2. Three step merges: the PR 7 merge, the PR 8 merge, upstream `main`. Each upstream point is first formatted with
   our `rustfmt.toml` on a temporary local branch, so conflicts show content changes only. One merge commit per step
   with its notable resolutions.
3. Port upstream's `model.rs` changes into `core/` (M2.1 moved the model there), separate commit.
4. Ratchet rebaseline for upstream-added code only, one commit, with a summary of what grew.
5. Golden pictures: run, re-bless only where upstream's intended drawing changes alter them, separate commit listing
   each case and the upstream commit.
6. `cargo xtask check --fast` after each step, `cargo xtask check` at the end.

Owner decisions applied:

- Both draw paths stay: upstream's Standard (multi-core fast path, default) and our batch path, with upstream's
  "Acquire > Draw path" setting. The draw-path acceptance plan in migration M9/M12: removal superseded, M12 measures both on the unit.
- Our acquisition correctness work stays (M2.x change detection and generation filter, kexec and watchdog, PCIe
  Gen1, self-heal, update hardening, SCPI empty reply and compound fixes). Where upstream fixed the same, one
  implementation stays; the Status names which.
- Upstream's module registries (decoders, measurements, analysers; their ADR code/0002) stay as they are.
- Modify/delete: upstream deleted fork specs, the unit doc and the unit rootfs files; ours stay (private, scrubbed
  at publish). Overhaul deleted per-crate `Cargo.lock` files and `docs/architecture.md`; overhaul's side stays.
- Open owner questions are recorded below and resolved conservatively (keep both).

## Status

- 2026-11-16: full gate on 633acfff: all steps pass except coverage (dsp 91.5 % against the floor 94.9 %, margin
  0.5) and python-tests (fixed in 3e316143). The floor stays (coordinator). 821 of 1118 uncovered dsp lines are in
  upstream files (motor, eye, measure, power, dpt, linecode, cphy, magnetic). Task 0078 (two jobs) adds behavioural
  tests for motor, dpt, linecode, cphy and power; the final full gate runs after they merge here.
- 2026-11-16: merge-caused inline measurement fixed (633acfff): raw windows above upstream's background threshold go
  to the worker, SCPI `:MEASure` queries wait up to 3 s and never answer a stale value; host-only hold path stays
  inline. Tests that counted steps or shared one deadline wait on their condition with a bounded deadline.

- 2026-11-16: goldens re-blessed in b33a5ebf: 621 captures in 263 cases, causes per region in the commit
  (upstream bars and corner button, zoom from the full record bf869f9b, Lanczos8 82d98eae, menus, measurement names,
  trigger level line). Batch trace drawing unchanged outside zoom.

Decisions (coordinator, 2026-11-16):

- Windows-only dependencies of upstream's desktop window (`minifb`, `windows-sys`) stay out; `ui/src/desktop.rs`
  builds only with `cfg(windows)`. A new dependency needs the owner's agreement.
- Ratchets: the retired upstream-rebaseline procedure (Git history) admits upstream findings in one rebaseline commit; crate keys (`impls ui Ui`) only where
  upstream added an inherent impl of that type in that crate.
- While a STOP view is shown, settings changes no longer reconfigure the acquisition until RUN (upstream behaviour,
  accepted).
- The STOP replay frame carries the identity it was captured with (our M2.3b rule), on top of upstream's re-read.
- `Model::cores` classes: ACQ, STATS, PHOSPHOR (a toggle reconfigures the acquisition).
- Software ADC core matching must apply on the batch path too (correctness fix); in the merge if small, else task
  0068.
- Golden `demo/normal/basic/02-auto` is re-blessed for upstream 9e7ad1d8 (trigger level line after a level change).
- Two eye implementations stay for this merge (our worker eye, upstream's eye on main with clock, rail, automatic
  mode and the SIM fingerprint). Follow-up task 0069: one eye with upstream's features on the measurement worker.
- `sim_*` fields regenerate the SIM record: class ACQ and STATS (VIEW only where they change presentation only).
- Upstream's measurement path in `measure_last` runs only for references and SIM or offline sources; live channels
  keep our generation-checked `measure_rec` alone (deep-record cost, task 0062). Not done in the merge: upstream's
  full-record gate, deskew, extremum timing and native HiRes features run on its channel jobs (12 tests). Follow-up
  task 0070 ports them into `measure_rec`; until then live channels are measured twice (ours first, generation
  checked; upstream's after it).
- Software ADC core matching on the batch path: not in the merge (the matcher learns per frame; the batch path
  decodes on parallel workers). Follow-up task 0068.
- Open owner questions (upstream behaviour kept until decided):
  1. SINGLE and statistics: our SINGLE resets statistics at the change point, upstream keeps them until the commit;
     two upstream SIM tests are ignored with this reason.
  2. The mixed-bus hardware path writes `Shared.want` directly, past our change point and publication.
  3. Upstream's `:MEASure` has no `:MEAS:CLE ITEMn` and no drop-oldest rule for a full item list.
- Also open: the host harness sends no mDNS announcement; worker-path Power for live channels lacks upstream's "why
  no numbers" rows.
- Socket tests on macOS loopback (`compatibility_hislip_*`, `compatibility_raw_tcp_pending_opc_*`, `raw_tcp_*`,
  `real_scpi_queries_*`) fail the same way on unmerged upstream 7c853752 on this host (6 of 18 selected): an
  upstream test race (one immediate non-blocking read), not the merge. Fixed in the merge if the harness read gets a
  bounded wait; otherwise follow-up task "socket test poll race". Never ignored to pass the gate.
- `Model` grows to 293 fields with upstream's fields (moved to `core/src/model/state.rs`); rebaselined, not
  regrouped. Follow-up task: regroup `Model` into sub-structs (ratchet back below 200).
- Open (owner): upstream's frame-0 and first-record drop (issue 5) runs beside our generation filter; they overlap
  on the first record after a change, unit check needed. `tools/device-deploy-ssh.sh` (upstream) duplicates our SSH
  `device-deploy.sh`; upstream's deploy defaults to telnet, ours stays SSH with the lease.
- Overhaul is merged into this branch at its tip before the full gate (integ-19 to integ-23).
- Follow-up tasks (ids to be allocated on the integration branch): 0068 software ADC core matching on the batch path;
  0069 one eye implementation with upstream's features on the measurement worker; 0070 one measurement path for live
  channels (port upstream's full-record gate, deskew, extremum timing and native HiRes into `measure_rec`); regroup
  `Model` into sub-structs (ratchet back below 200).

- 2026-11-16: ratchets rebaselined in 8259be01 (trailer `Ratchet-Rebaseline: upstream 2bd51214`): core size
  1/2 (added/raised), dsp clippy 49/5, dsp size 23/7, hal size 0/1, ui clippy 123/17, ui size 117/63, ui unsafe 6/1;
  crate keys `impls ui Ui` 33 -> 61, `impls ui Job` 4, `impls ui State` 5, `impls ui Worker` 7. Our own findings
  were fixed instead (core patch enums, a test cast). `cargo xtask ratchet` refuses while moves are listed: the
  integrator runs `cargo xtask ratchet --move` at the fast-forward.
- 2026-11-16: the retired upstream-rebaseline procedure (Git history) refined: crate keys only for inherent impls upstream added; moves of the integration carry
  attribution for upstream files relocated to new files and for continued key moves; the pre-commit gate accepts the
  rebaseline commit itself; admitted entries join the base after its moves.
- 2026-11-16: overhaul merged three times to its tip: 71f50eb8 (integ-19, integ-21), 0a09b4cf (integ-22, docs), 856f5f74
  (integ-23). M2.3d branches from d7418b0d. Upstream main had deleted the private `docs/fork/specs` 04 and 06 to 10
  without conflict; restored.
- 2026-11-16: step 3 (upstream main 2bd51214) committed as merge plus model port; 111 files conflicted raw, 47
  against the formatted base. Draw path: both paths, runtime choice through the change point, no drawpipe statics.
- 2026-11-16: step 2 (PR 8, 7c853752) committed as merge plus model port; 71 files conflicted raw, 28 against the
  formatted base. `docs/architecture.md` and per-crate `Cargo.lock` stay deleted (overhaul side). Upstream test
  literals (private addresses, a MAC, home paths) got documentation placeholders; `#[path]` modules moved to their
  default files (listed in `moves.txt`).
- 2026-11-16: step 1 (PR 7 merge 32be6a01) committed as merge plus `Model::cores` port; 12 files with real
  conflicts (38 hunks), 47 more formatting only. Upstream points are merged through copies formatted with our
  `rustfmt.toml`; each merge commit's second parent is the unformatted upstream commit.
- 2026-11-16: the retired upstream-rebaseline procedure (Git history) and its gate exception committed before the first upstream merge.
- 2026-11-16: bridge `-s ours` with upstream's PR 7 head bba435bc (scrubbed copy of b3539a34, already in overhaul).
- 2026-11-16: claimed on `integ-20-upstream` from `overhaul` c6f1f98.
