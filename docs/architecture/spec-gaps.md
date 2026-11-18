# Specification gaps

What [ADR 0038](../adr/0038-menus-and-parser.md) needs specified before its rebuilt parts start: the menu model, the
overview page, the SCPI parser, the setup keys M13 needs and the `:SYSTem:STATe?` schema. Settings, commands and
views keep the migration's design (`migration.md`) and need no separate spec. Counts are from `overhaul` dca35ed,
taken with grep; they size the legacy surface, they are not targets.

## What exists

| Area | Where | Covers | Lacks |
|---|---|---|---|
| Menus | `../../ui/README.md` (2026-11-03), `ui/src/ui/lint.rs` | start menu groups, dialog style, pickers and their item list, the layout lint | the tree, per-item ranges, defaults, visibility, knob behaviour; any rule for the active choice beyond segmented buttons and pickers |
| Overview | none | | everything |
| SCPI | `../../ui/README.md` (2026-11-02 subset), `extending.md` (Add an SCPI command), M15 | a subset list, the recipe, ordering of `:SINGLE;:WAV:DATA?` | a reference table for about 258 tokens |
| Setup files | `README.md`, Setup files | format, versioning, migration rules | the key catalogue (about 170 keys matched in `ui/src/store.rs`) |
| `:SYSTem:STATe?` | 40 mentions in the docs, unitlab | individual fields per task (for example task 0061) | a schema with a version |

## Gaps

### Menu model

- About 40 menus (`MenuId`, `ui/src/ui.rs` 331) and about 319 items (`Item`, `ui.rs` 424) exist only in code.
- Per item the spec needs: id, label, kind (choice, value, action, picker, sub menu, view selector), the setting or
  component it reads, the command it returns, choices or range with unit and step, default, visibility (model,
  option, mode), and what a tap, Multi 1 and Multi 2 do.
- **Active choice (owner).** One rule for every item that selects among options: segmented choices, pickers, and
  entries that open or select a view or mode (Bode, Eye/Jitter, Spectrum, the second pane, sources, analysis
  options). The active option is highlighted wherever the item is shown, including the start menu and sub-menu
  entries that lead to the active mode. Today only segmented buttons and pickers do it.
- Navigation: start menu, Back and close, focus after a picker, placement of dialogs, touch lock.

### Overview page (owner)

- Where it opens from (key, start menu, top bar).
- What it lists: acquisition (channels, time base, mode, depth), trigger, active analyses and views, measurements,
  math, mask, decoders. Inactive features omitted or shown as off.
- Whether a row opens its menu. Refresh on the `ChangeSet`, not per frame.
- It reads the menu model's value readers, so it shows the same values the menus do.

### SCPI reference table

- One row per command and query: path with short form, arguments, units, range, default, the `Command` or `Query` it
  maps to, errors, `*OPC` behaviour, Rigol compatibility note, model availability.
- Cross-cutting rules: error queue, ordering and consistency (`After { gen, epoch }`, M15), waveform formats,
  OOS extensions (`:SYSTem:STATe?`).
- A recorded session corpus for the differential oracle: unitlab, `ifload.py` and the openSciLab bridge traffic.
- VXI-11 maps to the same table. HTTP, MCP and the control FIFO stay as M15 specifies them.

### Setup keys, as far as M13 needs them

- The current keys per future settings group, with type, default and the field they set, so M13's round trip test
  has a reference and a renamed key gets its migration step.

### `:SYSTem:STATe?` schema

- Field list with units and the step or task that owns each field; a version that unitlab checks; the rule that a
  field is never renamed, only added.

## Documents to write

Docs only, in parallel with M2 to M12. Every requirement gets an id (`<doc>-<n>`). Codex workers extract the legacy
surface from code; each row is then marked keep, change (with the new rule) or drop, and the coordinator approves the
marks. The owner approves the menu tree, the highlight rule and the overview page.

| # | Document (`docs/spec/`) | Owner | Needed by |
|---|---|---|---|
| 1 | `menus.md`: menu tree, items, active-choice rule, navigation | codex worker (extract), coordinator (rules), owner approval | M17 menus |
| 2 | `overview.md`: the overview page | coordinator, owner approval | M17 menus |
| 3 | `scpi.md`: reference table, cross-cutting rules, session corpus | codex worker (table), coordinator (rules) | M15 |
| 4 | `setup-keys.md`: current keys per group | codex worker | M13 |
| 5 | `state-query.md`: `:SYSTem:STATe?` schema | coordinator | M15, unitlab |
