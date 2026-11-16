# 0017: Migrate through M12, then rebuild menus and the SCPI parser spec-first

Status: accepted. 2026-11-16 (coordinator, after an independent architecture review; the owner delegated the choice).

## Context

- The strangler migration (`../migration.md`) adds a bridge per step. M2 was planned at 7.5 host days and
  re-estimated at 12 to 15 in its task (`../../tasks/0003-m02-change-detection.md`). The normalize-to-`mark_changed`
  bridge stays in the shipping UI until M2.3e removes it.
- The hardware side carries behaviour only the unit proves: the watchdog and kexec budget (task 0054, adr/0015),
  PCIe Gen1 (task 0046), the FPGA register rules, roll, segments and SINGLE arming (task 0052), the M2.3b generation
  filter and the M2.3c completion path.
- The legacy `Board` trait takes `&Model` (`ui/src/board/mod.rs`, `Board::want`). The acquisition port of
  `../acquisition.md` names a board-neutral intent, but `AcqIntent` in `oos-core` borrows `Model`
  (`core/src/change/values.rs` 67).
- The menus are tables inside `ui/src/ui.rs` (11,233 lines): about 40 menus (`MenuId`, line 331) and about 319 items
  (`Item`, line 424). No menu tree exists outside code. The owner wants the active choice highlighted in every menu
  (for example the active view, Bode or Eye/Jitter) and one overview page of the active setup. Neither has a rule
  today.
- The SCPI parser (`ui/src/scpi.rs`, `ui/src/scpi/`) holds about 258 command tokens. The only reference is the
  2026-11-02 subset in `ui/README.md`.
- Task 0044 (`../../tasks/done/0044-spike-new-core.md`, report on branch `spike/new-core`) built a fresh core on the
  host. Its kernels took 49 and 183 us per frame against 13.9 and 7.7 us for the current pipeline. It estimated 74
  to 132 days to reach the current feature set and recommended the incremental migration.
- An independent architecture review compared the options in host days: the migration to M12 is about 45 planned
  and 70 to 90 adjusted; a full rebuild is 150 to 300 adjusted, with nothing of the first goal (a fast picture
  without hiccups or flicker) before the switch-over.

## Decision

1. **Migrate through M12 as planned**, M2.3d and M2.3e included, so the shipping UI leaves the bridges. 0.2.0 stays
   defined at M12 (`../../releases.md`).
2. **M5 takes an owned `AcqRequest`.** The change point produces an owned, board-neutral `AcqRequest` in `oos-core`
   instead of a borrow of `Model`. `Board` no longer takes `&Model`.
3. **After M12, split by layer:**
   - M13 and M14, settings groups and commands: migrate as planned.
   - M17 views: migrate as moves.
   - M17 menus: rebuild from a specification as a data-driven menu model. Items are data (label, kind, value
     reader, command, choices, visibility); one renderer draws them. The active-choice highlight is a property of
     the model, so every menu has it. The overview page is built on the same model.
   - M15 remote: rebuild the SCPI parser on `Command` and `Query` from a specification table. The legacy parser runs
     on the host as a differential oracle: every table row and a corpus of recorded sessions go through both, and
     each difference is either a recorded spec decision or a bug.
4. **Specification work starts now, docs only:** the menu tree with the highlight rule and the overview page (owner
   approval), and the SCPI reference table (`../spec-gaps.md`).
5. **Soft freeze** of the code these steps replace (`../../process/delivery.md`, Soft freeze).

## Consequences

- The first goal ships at M12 as planned, through steps that each have unit evidence.
- The menus and the parser are where migration would carry the most legacy shape for the least value: `ui.rs`
  tables and a hand-written parser without a reference. Rebuilding them costs a spec and removes one bridge per menu
  and per command family.
- The differential oracle checks the rebuilt parser without a hardware round per command family. It is a test
  dependency on legacy code, deleted with the legacy parser.
- `AcqRequest` makes the port testable with a fake that never sees `Model`, and lets M13 change the settings shape
  without touching the board.
- The spec documents are on the critical path of M15 and the M17 menus, not of M2 to M14.

## Rejected alternatives

**Full rebuild of UI and domain on the kept hardware adapter** (the first draft of this ADR). Rejected: 150 to 300
adjusted host days against 70 to 90 for the migration to M12; nothing of the first goal before a single switch-over;
task 0044 measured fresh kernels 3.5 to 24 times slower on the same input and estimated 74 to 132 days to feature
parity; the legacy UI would ship frozen, without the owner's requirements and the drawing fixes, for the whole period.

**Pure strangler for menus and remote.** Rejected for these two layers only: there is no specification to migrate
towards, so each step would specify the legacy behaviour, bugs included, as M2.2's shadow oracle had to; the owner's
highlight and overview requirements would be retrofitted into per-menu tables; the parser has no reference table to
test against, and migrating it command by command adds a bridge per family. Settings, commands and views keep the
strangler, because the migration already has their design and tests.
