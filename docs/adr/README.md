# Architecture decision records

One file per decision that shapes OOS: what was decided, why, and what follows from it. A record is short and stays as
it was written. A decision that changes gets a new record that supersedes the old one; the old one keeps its text and
gets the status "superseded by ADR NNNN".

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

## Index

| ADR | Category | Decision | Status |
|---|---|---|---|
| [0001](code/0001-three-crates-with-pure-hal-and-dsp.md) | code | Three crates; the hardware layer and the signal processing are pure functions | accepted |
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
| [0013](process/0013-no-vendor-code-only-libc.md) | process | No vendor code or data in the tree; `libc` is the only dependency | accepted |
| [0014](process/0014-refactorings-keep-every-pixel.md) | process | A refactoring keeps every pixel and every register write | accepted |
| [0015](process/0015-say-how-you-know.md) | process | Every statement about the hardware says how it is known | accepted |

## Writing one

1. Copy [template.md](template.md) into the category's folder as `NNNN-short-title.md`. `NNNN` is the next free number
   over all categories, so a number names one record wherever it is filed.
2. Fill it in: the context (what forced a decision, the measurements), the decision in one or two sentences, the
   alternatives that were looked at and why they lost, the consequences (also the unwelcome ones).
3. Add its line to the index above, in the same pull request as the change it belongs to.

Status values: `proposed` (under discussion), `accepted`, `accepted, to be reviewed` (with the condition for the review
in the record), `superseded by ADR NNNN`, `rejected` (kept, so the idea is not tried a second time without new facts).

The rules of the tree hold here too ([CONTRIBUTING.md](../../CONTRIBUTING.md)): English, no vendor code or data, and
for every statement about the hardware how it is known.
