# Development: the gate

Every change passes one gate before it is committed, pushed or merged:

```sh
cargo xtask check                  # the full gate
cargo xtask check --fast           # partial, seconds: formatting, size limits, statics, unsafe, architecture
cargo xtask check --no-coverage    # partial: the full gate without the coverage run
```

A change is done when the full gate passes on its final commit. `--fast` and `--no-coverage` are partial results.

Run it from the repository root. It prints one line per step with its time, the details of a failure, and exits
non-zero when a step failed. Humans and agents run the same command; the hooks and CI run it too.

## Setup

- Rust stable with `rustfmt` and `clippy`, and the scope's target: `rustup target add aarch64-unknown-linux-musl`.
- Coverage: `cargo install cargo-llvm-cov` and `rustup component add llvm-tools-preview`. Without them the full gate
  fails at the coverage step; `--no-coverage` skips it.
- Hooks, once per clone: `git config core.hooksPath .githooks`. pre-commit runs `check --fast`, pre-push the full
  gate.
- The ratchet files are compared with the base branch `overhaul` (or `origin/overhaul`): the clone needs it
  (`git fetch origin overhaul`; CI checks out with full history).
- Every cargo call that resolves dependencies uses `--locked` (the `cargo xtask` alias, clippy, tests, metadata,
  coverage and `tools/perf-guard.sh`): a change of a dependency comes with its `Cargo.lock` change.

## Privacy checks

Before committing, run `python3 tools/privacy/scan.py --staged` and
`python3 tools/privacy/scan.py --message target/COMMIT_MSG.txt`. The stdlib scanner checks the index and commit message;
the separate Privacy CI job checks every incoming commit's additions and message. Findings redact values.
Use documentation addresses and placeholders in examples, and configure device tools explicitly with `--host`
or `OOS_SCOPE` where supported. Never infer a real unit address from a checked-in example.

Keep private strings only in the ignored `local/privacy-literals.txt`, one exact value per line. Reviewed synthetic
exceptions key the exact path, rule and matched-value hash in `tools/privacy/allowlist.json`. See [the scanner instructions](../tools/privacy/README.md)
for commands, limitations, the policy format and the pending hook integration. The xtask hook point waits for
G1/task 0043; this check does not replace the full gate.

## The crates

One root workspace (`Cargo.toml`): `hal`, `dsp`, `ui`, `fpgad` and `xtask`, with one `Cargo.lock`, one `target/`
and the release profile of the scope binaries. `gpu` is C (`gpu/oos-gpu.c`), built in its own container by
`gpu/build.sh` and `gpu/test-host.sh`, so it is no member. The scope's binaries land in
`target/aarch64-unknown-linux-musl/release/`; the target flags (rust-lld, static, Cortex-A72) are in the root
`.cargo/config.toml`. The gate runs each step crate by crate (`CRATES` in `xtask/src/repo.rs`), so its counts and
ratchets stay per crate; a workspace member without an entry there fails the architecture step. `rustfmt.toml` and
`clippy.toml` sit at the root.

The syntax checks (size, statics, unsafe, module rules) read every file reachable from a member's cargo targets
through its `mod` declarations, inline modules included: library, binaries, integration tests, benches, examples.
They refuse what they cannot follow: `#[path]` on a module (also inside `cfg_attr`) and a `mod x;` inside a function
body. Test code is a test, bench or example target, a `#[cfg(test)]` module (in its own file too), or a `#[test]`
function; a file reached both as test and as production code is production code.

## What each check enforces

| Step | Command | Enforces | Why |
|---|---|---|---|
| fmt | `cargo fmt --check` per crate | `rustfmt.toml`: 120 columns, max heuristics | diffs show changes, not style |
| size limits | syntax walk (`syn`) | file ≤ 400 lines without `#[cfg(test)]` items, fn ≤ 60 lines, struct ≤ 12 fields, a type's inherent impls in ≤ 3 files | small units; the `Ui` of 155 fields over 33 files is what this stops |
| mutable statics | syntax walk | a `static` whose type holds a `Mutex`, `RwLock`, atomic or `Cell`/`RefCell`/`UnsafeCell` (also through the crate's own types) only as a metrics counter (`static` lines in `architecture.txt`); `OnceLock`, `LazyLock` and `OnceCell` around plain data are allowed (set once, then read only) | global mutable state hides ownership |
| unsafe | syntax walk | `unsafe` blocks, functions, impls, traits, extern blocks and attributes (`#[unsafe(..)]`, `no_mangle`, `export_name`, `link_section`) only under the `unsafe` path prefixes of `architecture.txt`: oos-hal and the adapters of ui; `dsp` has `#![forbid(unsafe_code)]` | memory safety stays at the hardware edge (AGENTS.md) |
| architecture | `cargo metadata`, syntax walk | `xtask/data/architecture.txt`: allowed crate dependencies; module rules inside a crate | the layers cannot erode by accident |
| clippy | `cargo clippy --all-targets`, for the host and for `aarch64-unknown-linux-musl` | the `[lints]` table of each crate and `clippy.toml`; the scope build compiles | correctness, idiom, size and complexity, also in Linux-only code |
| tests (debug, release) | `cargo test`, `cargo test --release` | every test passes in both profiles, full counts | release-only bugs (overflow checks, timing) show |
| perf guards | `tools/perf-guard.sh` | the fork's allocation counts and timing ratios; its exit code decides | the fast paths stay fast |
| coverage | `cargo llvm-cov` per crate | line coverage per crate ≥ the floor in `xtask/data/coverage.txt` | coverage does not fall |

The limits have no exemptions (ADR 0010): a record over 12 fields groups its fields into sub-structs.

Size limits measure lines from `fn` to the closing brace, including comments and blank lines. Test code is exempt.
Clippy's `too_many_lines` (60 code lines, `clippy.toml`) covers test code too.

The lint set (identical in every crate, `[lints]` in its `Cargo.toml`): `clippy::all`, plus `too_many_lines`,
`cognitive_complexity`, `too_many_arguments`, `type_complexity`, `large_enum_variant`, `undocumented_unsafe_blocks`
(every `unsafe` block has a `// SAFETY:` comment), `missing_safety_doc` and `unwrap_used` (outside
tests: a panic belongs in an `expect` with a reason, or the error is handled). For a `Mutex` use
`oos_hal::sync::lock(&m)`: it takes the data of a poisoned lock, which only a test can produce since the binaries
abort on panic. rustc: the `unused` group, and
`missing_debug_implementations` in the library crates `hal` and `dsp`. Every lint is a warning to rustc and clippy, so
an ordinary build still runs; the gate turns findings into failures through the ratchet. At `deny`, clippy would
stop at the first crate with an old finding and count nothing after it. `xtask` itself denies all of
them and has no ratchet.

The scope's target runs as a second clippy pass per scope crate, on the host: clippy does not link, so the
target's standard library is enough. It compiles the same crates with the same (default) features, target and target
flags (`.cargo/config.toml`) as the image build (`boot/build-dev-image.sh`), without code generation and linking, and
lints the code that only builds there (`cfg(target_os = "linux")`). A finding seen in both passes counts once. The
earlier Docker check mounted every worktree at the same path and could take another tree's build as fresh.

## Ratchets

The rules came in with the code already over them. Those findings are listed, each with its current size or count:

- `xtask/data/size.txt`: `<size> <kind> <file or crate> <name>`, every item over a size limit.
- `xtask/data/clippy.txt`: `<count> <crate> <file> <lint>`, every lint finding.
- `xtask/data/statics.txt`: `1 static <file> <name>`, every mutable static that is not a metric.
- `xtask/data/unsafe.txt`: `<count> unsafe <file>`, `unsafe` outside the allowed paths.
- `xtask/data/coverage.txt`: `<floor> <crate>`, line coverage in tenths of a percent.

The gate fails when a listed entry grows, when an entry appears that is not listed (a new oversized function, a new
lint finding in a file), and when coverage falls below its floor. An entry that shrank passes, and the gate says so.
Write the improvement back:

```sh
cargo xtask ratchet               # lower sizes and counts to the current values, drop fixed entries
cargo xtask ratchet --coverage    # also raise the coverage floors to the current values
```

`ratchet` never raises a size or count and never adds an entry: a new finding is fixed, not listed. Branches do not
edit the ratchet and coverage files: the integrator runs `cargo xtask ratchet` at merge, so two branches never
conflict on them ([migration rules](architecture/migration.md#rules-for-every-step), Shared ratchet files).
`cargo xtask ratchet --init` writes the files from the current state; it was run when the rules came in.

### Base trust

The gate compares each ratchet file with its version at the merge base of `HEAD` and `overhaul` (`origin/overhaul`
in a CI checkout), or `HEAD^1` when `HEAD` is on `overhaul` itself. An entry that is not there, a larger value, or a
removed or lowered coverage floor fails. Any git error fails. `--init` refuses a ratchet file the base already has.

- A ratchet file absent at the base is legal only while the base has no `xtask`: the one merge that brings the gate
  in. After it, a missing ratchet file fails.
- CI (GitHub Actions, the base of a pull request is its target branch) is the authority. Locally the gate trusts the
  `overhaul` ref; setting `CI` or `OOS_GATE_BASE` by hand, or moving `overhaul` to pass a check, breaks the rules.
- On the integration branch, `HEAD^1` covers one commit. A push of several commits is not compared commit by commit:
  the integrator runs the full gate on the branch before the fast-forward.

Lint counts are per file, not per place: fixing one `unwrap` and adding another in the same file passes. That is the
price of a ratchet that survives line moves. Size entries name the item, so they follow it through moves inside the
file; a renamed or moved item counts as new.

Planned in gate step G1 ([migration](architecture/migration.md#g1-gate-prerequisites)), before any migration step
that moves code:
- move-aware validation: a branch lists its planned moves in `xtask/data/moves.txt` (`<old> -> <new>`, part of the
  step, not a ratchet file); the branch gate accepts a listed item at its new place if it is not larger than at the
  merge base, and the integrator transfers the entries with `cargo xtask ratchet --move` at merge;
- a coverage margin of 0.5 points per branch, raised by the integrator after the merge;
- the function limit counted in code lines only;
- module rules over external crate paths;
- a shrink-only count of `#[allow(clippy::disallowed_methods)]` per crate;
- the known-failure listing (`#[ignore = "known: task NNNN"]`), shrink-only.

## Changing a rule

- A new crate: add it to `CRATES` in `xtask/src/repo.rs` (with `ratchet: false`, so it starts clean) and a `crate`
  line to `xtask/data/architecture.txt`.
- A new dependency: extend its crate's line in `xtask/data/architecture.txt`. The gate fails until the line allows it,
  so the decision is visible in review.
- Module rules in `xtask/data/architecture.txt`, over module paths such as `board` or `board::dho`. Every path in
  the code counts (`use` trees, calls, types), with `self::` and `super::` resolved against the file's module and any
  inline `mod`. A module is inside a rule's module when its path starts with it segment-wise; for a file, the most
  specific `layer` rule applies.
  - `forbid <crate dir> <from> <to>`: code under `<crate dir>/src/<from>.rs` or `src/<from>/` must not name
    `crate::<to>`.
  - `layer <crate dir> <module> -> <modules...>`: code of `<module>` may name only the listed modules and itself;
    nothing after `->` makes it a leaf.

  Module rules have no ratchet: the tree must meet a rule when it is added. The target rules for `ui` are in
  [Enforcement](architecture/README.md#enforcement); each migration step adds the ones its modules meet.
- The statics check reads the type as written and follows the structs, enums, type aliases and `use ... as`
  renames of the crate it names (not associated types, `<T as Trait>::X`): `OnceLock<Counters>` counts when `Counters` holds atomics, `static S: M<u8>` counts
  after `use std::sync::Mutex as M`. Types match by name, so two types of the same name in one crate count as one.
- A metrics counter as a `static`: `static <file> <name>` in `xtask/data/architecture.txt`, with `<name>` as the
  gate prints it (`<function>::NAME` for a static inside a function). A listed static that no longer exists fails.
- Where `unsafe` may appear: `unsafe <path prefix>` in `xtask/data/architecture.txt` (a crate directory, a
  directory, or a file; `x` also covers `x.rs`). The list moves with the migration.
- A limit: the constants in `xtask/src/size/mod.rs` and the thresholds in `clippy.toml`. Raising a limit lets
  findings go: `cargo xtask ratchet` writes the decreases. Lowering a limit makes findings the ratchet files do not
  list, and they cannot be added (`--init` refuses existing files): the change that lowers a limit fixes them.

## Known limits

Not enforced yet; each is a task.

- Macros: the syntax checks read unexpanded code. Items a `macro_rules!` or `thread_local!` generates (a static, a
  struct, a module path, an `unsafe` block) are not seen. Clippy sees expanded code.
- Module boundaries: an inline `mod` takes the rules of its file's module, not its own; a glob import (`use
  super::*`) followed by a bare name, and a re-export through an allowed module, hide the module the name comes from.
- Impl spread counts the spelling of the self type (its last path segment): aliases split one type, and two types of
  the same name merge.
- Path dependencies are checked by package name only, not by source: a local fork of an allowed crate passes.
- Types imported from another workspace crate, and associated types, are not followed by the statics check.
- The base branch is the local `overhaul` ref; CI is the authority on it.
- pre-commit checks the working tree, not the staged index, and only the fast part. The done rule is the full gate
  on the final commit.
