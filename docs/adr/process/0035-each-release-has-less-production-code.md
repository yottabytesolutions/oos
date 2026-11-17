# ADR 0035: Each release strives for less production code than the one before

- Status: proposed
- Date: 2026-11-17
- Category: process

## Context

The tree grows fast. Generated and hastily written code adds lines quicker than anyone can read them, and every
line is a place for a bug. Good abstractions and reuse keep the tree legible. Nothing measured the size of the code
as a whole: the size ratchet (ADR 0039, [architecture/0010](../../architecture/adr/0010-size-limits.md)) limits
files, functions and structs one by one, so ten new files of 300 lines pass it.

The file limit also counted comment lines. A file near 400 lines lost room for each line of documentation, so the
limit pushed against the comments that keep code legible.

On 2026-11-17 (`cargo xtask loc` on `ci/t0095-stabilize-main`) the production code is 111350 lines: core 7070,
dsp 14120, fpgad 2658, hal 14419, ui 73083. Upstream v0.1.13 had 82085.

## Decision

- Every release strives for fewer production code lines than the previous release. Prefer extending an existing
  abstraction and reusing code over a parallel one.
- A release that grows production code says why in a paragraph that starts with `Code growth:`, in the release tag's
  annotation or in its `CHANGELOG.md` section. The release workflow fails without it.
- Production code lines are the lines with a Rust token on them in the crates built for the scope (`target: true` in
  `CRATES`, `xtask/src/repo.rs`), outside test code. Not counted:
  - blank lines, comments and doc comments;
  - test code: `#[cfg(test)]` items and `#[test]` functions with their attributes, files below a `#[cfg(test)]`
    module, and the test, bench and example targets (`tests/`, `benches/`, `examples/`);
  - generated files: a file with `@generated` in its first five lines;
  - golden pictures, fixtures and vendored data: they are no Rust modules, so the count never reaches them.
- `cargo xtask loc` prints the count per crate and in total. Beside it, as tooling: `xtask` (host-only Rust) and the
  shell and Python lines (neither blank nor a `#` comment) outside test directories and test files. Tooling is
  reported, not held to the goal. `--json` gives the same numbers for a script, `--compare <git-ref>` the change
  per crate since `ref`.
- Tests and documentation are not counted, so nobody trims them to meet the goal. A change that deletes tests or
  comments to shrink the number is a review finding.
- The file size limit stops counting comment lines, doc comments included: documentation never competes with code
  for a file's 400 lines. Blank lines still count. The function limit already counted only code lines. The ratchet
  values only fall with this change.
- Enforcement:
  - Every pull request shows the production code change against its base in the `fast` job's summary. It is
    informational and never fails: a feature may add lines; the release is what must shrink.
  - The release workflow compares the tag with the previous `v*` tag, puts the table in the draft release notes and
    fails the verify step when production code grew and no `Code growth:` paragraph explains it.
  - Reviewers flag duplicated logic, near-copies and missing reuse (`AGENTS.md`, Code).

## Alternatives

- **A hard per-PR budget:** blocks features that need lines and pushes code into fewer, longer lines. The release
  is the unit that has to shrink.
- **Count physical lines (`wc -l`, `tokei`):** counts comments and tests, so the cheapest way to meet the goal would
  be to delete documentation and tests. `tokei` is also a new dependency and does not know `#[cfg(test)]`.
- **Count `xtask` and the scripts as production code:** a stricter gate or a better test harness would then compete
  with the product. They are reported so their growth stays visible.
- **Leave blank lines out of the file limit too:** a larger loosening than the issue asked for; blank lines are no
  documentation.

## Consequences

- The release notes carry the size of the product and its change; growth needs a reason a reader can judge.
- Shell scripts that run on the scope (`rootfs/`) are reported as tooling with the other scripts. C code (`gpu/`)
  is not counted.
- Python docstrings are counted as Python lines; only `#` comments are left out.
- To measure the releases before the root workspace (up to v0.1.13), `cargo xtask loc` reads each top-level crate as
  its own workspace and follows a plain `#[path]`; the gate still refuses `#[path]`.
- Worth a second look when the count stops reflecting legibility, for example when code moves into generated files
  or data tables to leave the count.

## References

`xtask/src/loc/`, `xtask/src/size/visit.rs`, `.github/workflows/pr.yml`, `.github/workflows/release.yml`,
`tools/ci/code_growth.py`, [docs/development.md](../../development.md), issue #19, ADR 0039,
architecture/0010.
