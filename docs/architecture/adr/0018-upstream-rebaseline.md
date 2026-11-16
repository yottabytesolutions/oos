# 0018: Upstream merges rebaseline ratchets

Status: accepted. 2026-11-16 (coordinator, on the owner's decision to merge upstream regularly).

## Context

- The owner decided to merge upstream (deckerjulian/oos) into `overhaul` regularly (task 0066).
- The ratchet files may only shrink. The gate compares each with its version at the merge base with `overhaul`
  and fails on an added key or a raised value (`xtask/src/base.rs`, `widened`).
- Upstream does not follow our size and lint limits. Its code arrives with new oversized items, new lint findings and
  new statics. The first intake (PR 7, PR 8 and upstream `main`, about 53,000 added lines) cannot pass the gate.
- Refactoring upstream code inside a merge mixes their change with ours, makes the next merge harder and is not
  ours to decide.

## Decision

Ratchet entries may be added or raised only in a dedicated rebaseline commit that

1. touches only `xtask/data/**`;
2. carries the trailer `Ratchet-Rebaseline: upstream <sha>`;
3. has a merge of that upstream commit (or of an ancestor of it) as an ancestor inside the same integration, after
   the protected base; a bridge merge that keeps our tree (strategy ours) does not count;
4. adds or raises only keys whose file was changed by those upstream merges (the diff between each merge's base and
   its upstream parent). The one crate key without a file, `impls <crate> <Type>` (impl blocks of a type in more files than
   allowed), counts only when those merges added an `impl` block of that type in that crate. A key in our code counts only when a move recorded in the base's `xtask/data/moves.txt` carried it out of
   such a file (M2.1 moved `Model` from `ui/src/model.rs` to `core/src/model/`), or a later move in the
   integration's `moves.txt` carried on: a file move of an upstream file to a new file (a `#[path]` module to its
   default module file) or a key move that continues a listed move. A destination no such move names fails, and so
   does a file move onto a file the base already has.

An upstream merge is a merge after the base that changes the tree and whose own merge base is not in the base: a
task branch starts from the base, upstream does not.

Between an upstream merge and its rebaseline commit, the fast gate (`check --fast`, the pre-commit hook) reports
the findings rule 4 would cover as pending rebaseline, with a count per crate and ratchet, instead of failing. Once
a rebaseline commit names that upstream commit or a later one, nothing is pending. The full gate never holds
findings as pending.

The gate adds what such a commit admits to the base before the comparison (`xtask/src/rebaseline.rs`). A rebaseline
commit that breaks a rule fails the gate. A commit without the trailer admits nothing. Our own code never gets a new
allowance this way; existing entries for our code may only shrink. `cargo xtask ratchet` still never raises.

## Consequences

- Upstream intake passes the gate without touching upstream code. The rebaseline commit lists what grew, so review
  sees the debt that came in.
- A file that both sides change (`ui/src/ui.rs`) can grow in a rebaseline. That is the price of merging upstream; the
  rebaseline commit's summary names these files.
- Crate keys are attributed by what upstream added: a new `impl Ui` block in an upstream file admits a larger
  `impls ui Ui`. The rebaseline commit lists admitted crate keys separately.
- The merge commits and the commits between them and the rebaseline pass the pre-commit hook without `--no-verify`;
  the pre-push and completion gate still require the rebaseline commit.
- The exception is mechanical and checked; it does not loosen a limit or a lint.
