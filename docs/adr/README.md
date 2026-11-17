# Architecture decision records

One file per decision that shapes OOS: what was decided, why, and what follows from it. A record is short and stays as
it was written. A decision that changes gets a new record that supersedes the old one. The old file is deleted; its
line in the index stays with the record that replaced it, and its text stays in the history
(`git log --diff-filter=D -- docs/adr/`).

The other documents say how things work now. These records say why they are that way, so nobody has to find it out
again from the history.

## Categories

| Folder | What belongs there |
|---|---|
| [code/](code/) | structure of the software: crates, modules, threads, processes, data paths |
| [ui-ux/](ui-ux/) | the screen: layout, operation by touch, keys and knobs, look |
| [hardware/](hardware/) | how OOS treats the scope's hardware: FPGA, front ends, storage, what it never touches |
| [system/](system/) | boot, the card, updates, network, the image |
| [process/](process/) | how the project works: tests, guards, what may enter the tree, documentation |

A decision that touches two categories goes where its consequences are largest and names the other in its text.

## Series

This is the one series for new records. It continues at 0030; 0027 to 0029 are not used (ADR 0030). The
records of the restructuring are in [docs/architecture/adr/](../architecture/adr/README.md), closed to new records,
and cited as `architecture/NNNN`.

## Index

| ADR | Category | Decision | Status |
|---|---|---|---|
| [0001](code/0001-three-crates-with-pure-hal-and-dsp.md) | code | Three crates; the hardware layer and the signal processing are pure functions | accepted, amended by architecture/0014 |
| [0002](code/0002-extension-by-registered-modules.md) | code | Decoders, measurements and analyser pages are modules behind one registration | accepted |
| [0003](code/0003-gpu-drawing-in-a-helper-process.md) | code | The GPU draws from commands in a helper process; the CPU stays the fallback | accepted |
| [0004](code/0004-standard-draw-path-by-default.md) | code | Two draw paths: Standard by default, Batch to choose | accepted, to be reviewed |
| [0005](ui-ux/0005-one-measurement-page.md) | ui-ux | One measurement page; one tap adds or removes | accepted |
| [0006](ui-ux/0006-lists-scroll-by-bar-and-finger.md) | ui-ux | Lists scroll by a scroll bar and by the finger, not by buttons | accepted |
| [0007](ui-ux/0007-top-bar-with-four-entries.md) | ui-ux | The top bar holds four entries; everything else starts from the start menu | accepted |
| [0008](ui-ux/0008-own-visual-style.md) | ui-ux | An own look: neutral colours, white accent, round shapes, no empty areas | accepted |
| [0009](hardware/0009-areas-oos-never-writes.md) | hardware | OOS never writes the FPGA flash, `/rigol/data`, `uboot`, `trust` or `parameter` | accepted |
| [0010](hardware/0010-unverified-hardware-functions-are-opt-in.md) | hardware | A hardware function that is not verified on a unit is off by default | accepted |
| [0011](system/0011-oos-in-recovery-stock-stays-bootable.md) | system | OOS lives in `recovery`; the stock system stays on the card and bootable | accepted |
| [0012](system/0012-signed-updates-with-trial-start-and-rollback.md) | system | Updates are signed, started on trial and rolled back when they fail | accepted |
| [0013](process/0013-no-vendor-material-in-the-tree.md) | process | No vendor material in the tree | accepted |
| [0014](process/0014-refactorings-keep-every-pixel.md) | process | A refactoring keeps every pixel and every register write | accepted |
| [0015](process/0015-say-how-you-know.md) | process | Every statement about the hardware says how it is known | accepted |
| 0016 | process | Rust code is formatted by rustfmt with the tree's settings | superseded by ADR 0039 |
| 0017 | process | Clippy warnings are errors, with a short list of allowed lints | superseded by ADR 0039 |
| [0018](process/0018-one-agents-md-for-people-and-agents.md) | process | One `AGENTS.md` holds the instructions for coding agents; it points to the rules | accepted, amended 2026-11-17 |
| [0019](process/0019-one-style-for-text-files-and-commits.md) | process | One style for files, documents and commits, checked where a tool can | accepted, amended 2026-11-17 |
| 0020 | process | The checks come in layers; `tools/check.sh` runs every host layer | superseded by ADR 0031 |
| 0021 | process | What runs on the scope depends on `libc` only; a desktop build may use its platform's crates | superseded by ADR 0037 |
| [0022](code/0022-one-scpi-interpreter-transports-only-frame.md) | code | One SCPI interpreter in the frame loop; every remote transport only frames its messages | accepted, amended by ADR 0037 |
| [0023](code/0023-scpi-stock-names-first-oos-identifies-as-oos.md) | code | SCPI follows the stock command names where they exist; OOS identifies itself as OOS | accepted |
| [0024](system/0024-vxi11-on-the-scope-hislip-on-a-computer-own-discovery.md) | system | VXI-11 on the scope, HiSLIP on a computer; found by mDNS / DNS-SD and LXI identification | accepted, amended 2026-11-17 |
| [0025](system/0025-remote-services-are-bench-lan-services.md) | system | Remote services are unauthenticated bench-LAN services, each with a switch, limited by `net.allow` | accepted |
| 0026 | process | A contribution is signed off, holds no vendor material and enters main as reviewed commits | superseded by ADR 0040 |
| [0030](process/0030-this-tree-is-a-hard-fork-of-oos.md) | process | This tree is a hard fork of OOS; upstream enters only by cherry-pick | accepted |
| [0031](process/0031-the-gate-is-cargo-xtask-check.md) | process | The gate is `cargo xtask check` | accepted, amended 2026-11-17 |
| 0032 | process | Lints and limits are per crate and ratchet down only | superseded by ADR 0039 |
| [0033](process/0033-development-platforms.md) | process | Development runs on Linux, the BSDs and macOS; Windows users develop in WSL or a Linux VM; Rust tooling goes into `xtask` | accepted, amended by ADR 0036 |
| [0034](process/0034-multi-pr-work-lands-as-a-stack.md) | process | Work that spans more than one pull request lands as a stack | proposed |
| [0035](process/0035-each-release-has-less-production-code.md) | process | Each release strives for less production code than the one before | proposed |
| [0036](code/0036-the-desktop-view-is-the-browser-ui.md) | code | The desktop view is the browser UI behind `oos-ui --serve`; the native Windows window is its own project | accepted |
| [0037](code/0037-crates-where-they-remove-code-remote-io-on-tokio.md) | code | Crates where they remove code or blocking; remote IO runs async on tokio | accepted |
| [0038](code/0038-one-table-may-generate-per-field-code.md) | code | One declarative table may generate per-field code when tests cover every item | accepted |
| [0039](process/0039-format-lints-and-ratchets-the-author-tightens.md) | process | rustfmt formats; lints and limits ratchet down; the author of a change tightens | accepted |
| [0040](process/0040-contributions-signed-off-no-vendor-material-rebased.md) | process | A contribution is signed off, holds no vendor material and lands rebased through the merge queue | accepted |

## Reading order for agents

An agent (or a new contributor) reads [AGENTS.md](../../AGENTS.md) first, then this index, then the records that
touch the change: ADR 0009, 0010 and 0015 for anything near the hardware, ADR 0014 and 0031 for how to show that a
change is safe, ADR 0039 for formatting, lints and ratchets, ADR 0018 and 0019 for how code and text look, ADR 0013
and ADR 0040 for what may enter the tree, ADR 0037 for dependencies and remote IO, ADR 0022 to 0025 for SCPI, VXI-11, HiSLIP and
the other network services, ADR 0030 for what this tree is, ADR 0033 for the hosts development runs on. The records
are short on purpose; reading the ones that apply costs minutes and saves a review round.

## Writing one

1. Copy [template.md](template.md) into the category's folder as `NNNN-short-title.md`. `NNNN` is the next free number
   over all categories, so a number names one record wherever it is filed.
2. Fill it in: the context (what forced a decision, the measurements), the decision in one or two sentences, the
   alternatives that were looked at and why they lost, the consequences (also the unwelcome ones).
3. Add its line to the index above, in the same pull request as the change it belongs to.

Status values: `proposed` (under discussion), `accepted`, `accepted, to be reviewed` (with the condition for the review
in the record), `accepted, amended by ADR NNNN` (it holds, with a limit or an exception that the newer record
states), `superseded by ADR NNNN` (file deleted, index line kept), `rejected` (kept, so the idea is not tried a
second time without new facts).

A record is not rewritten when the decision changes. Two kinds of change are made in place, with the date: a
correction that leaves the decision as it was (a file that moved, a name that changed, a case the record missed while
the code already had it), and a list the record keeps up to date (such as the keys of ADR 0010). Anything else is a
new record.

The rules of the tree hold here too ([CONTRIBUTING.md](../../CONTRIBUTING.md)): English, no vendor code or data, and
for every statement about the hardware how it is known.

A new rule that agents must follow gets one line in [AGENTS.md](../../AGENTS.md) that names its record (ADR 0018).

To disagree with a record, open an issue or a pull request against it. A record that is `proposed` is the cheapest
place to change course: nothing is built on it yet.
