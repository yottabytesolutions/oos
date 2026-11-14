# Development: the gate

Every change passes one gate before it is committed, pushed or merged:

```sh
cargo xtask check                  # the full gate
cargo xtask check --fast           # seconds: formatting, size limits, architecture
cargo xtask check --no-coverage    # the full gate without the coverage run
```

Run it from the repository root. It prints one line per step with its time, the details of a failure, and exits
non-zero when a step failed. Humans and agents run the same command; the hooks and CI run it too.

## Setup

- Rust stable with `rustfmt` and `clippy`, and the scope's target: `rustup target add aarch64-unknown-linux-musl`.
- Coverage: `cargo install cargo-llvm-cov` and `rustup component add llvm-tools-preview`. Without them the full gate
  fails at the coverage step; `--no-coverage` skips it.
- Hooks, once per clone: `git config core.hooksPath .githooks`. pre-commit runs `check --fast`, pre-push the full
  gate.

## The crates

`hal`, `dsp`, `ui` and `fpgad` build separately, each with its own `Cargo.lock`. `gpu` is C (`gpu/oos-gpu.c`), not a
crate. `xtask` is the gate itself. There is no root workspace, on purpose: the build scripts, the image build and the
GPU test expect each crate's own `target/` (`ui/target/aarch64-unknown-linux-musl/release/oos-ui`), the crates have
their own release profiles, and upstream has the same layout. A root workspace would move every target directory and
merge the lock files for one shared build cache. That can come later as its own change. Until then `cargo xtask`
iterates over the crates (`xtask/src/repo.rs`). `rustfmt.toml` and `clippy.toml` sit at the root; rustfmt and clippy
find them from every crate.

## What each check enforces

| Step | Command | Enforces | Why |
|---|---|---|---|
| fmt | `cargo fmt --check` per crate | `rustfmt.toml`: 120 columns, max heuristics | diffs show changes, not style |
| size limits | syntax walk (`syn`) | file ≤ 400 lines without `#[cfg(test)]` items, fn ≤ 60 lines, struct ≤ 12 fields, a type's inherent impls in ≤ 3 files | small units; the `Ui` of 155 fields over 33 files is what this stops |
| mutable statics | syntax walk | a `static` whose type holds a `Mutex`, `RwLock`, atomic or `Cell`/`RefCell`/`UnsafeCell` (also through the crate's own types) only as a metrics counter (`static` lines in `architecture.txt`); `OnceLock`, `LazyLock` and `OnceCell` around plain data are allowed (set once, then read only) | global mutable state hides ownership |
| unsafe | syntax walk | `unsafe` blocks, functions, impls and traits only under the `unsafe` path prefixes of `architecture.txt`: oos-hal and the adapters of ui; `dsp` has `#![forbid(unsafe_code)]` | memory safety stays at the hardware edge (AGENTS.md) |
| architecture | `cargo metadata`, syntax walk | `xtask/data/architecture.txt`: allowed crate dependencies; module rules inside a crate | the layers cannot erode by accident |
| clippy | `cargo clippy --all-targets` | the `[lints]` table of each crate and `clippy.toml` | correctness, idiom, size and complexity |
| tests (debug, release) | `cargo test`, `cargo test --release` | every test passes in both profiles, full counts | release-only bugs (overflow checks, timing) show |
| scope target | `cargo check --all-targets --target aarch64-unknown-linux-musl` | the scope build compiles, `cfg(target_os = "linux")` code included | the host build hides Linux-only code |
| perf guards | `tools/perf-guard.sh` | the fork's allocation counts and timing ratios; its exit code decides | the fast paths stay fast |
| coverage | `cargo llvm-cov` per crate | line coverage per crate ≥ the floor in `xtask/data/coverage.txt` | coverage does not fall |

The limits have no exemptions (ADR 0010): a record over 12 fields groups its fields into sub-structs.

Size limits measure lines from `fn` to the closing brace, including comments and blank lines. Test code is exempt:
`#[cfg(test)]` modules, `#[test]` functions and the crates' `tests/` directories. Clippy's `too_many_lines` (60 code
lines, `clippy.toml`) covers test code too.

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

The scope check runs on the host: `cargo check` does not link, so the target's standard library is enough. It
compiles the same crates with the same (default) features, target and target flags (`.cargo/config.toml`) as the
image build (`boot/build-dev-image.sh`), only without code generation and linking. Its own
target directory (`target/xtask-arm64` at the root) keeps it apart from the host build. The earlier Docker check
mounted every worktree at the same path and could take another tree's build as fresh.

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

`ratchet` never raises a size or count and never adds an entry: a new finding is fixed, not listed. Commit the
changed files with the change that earned them. `cargo xtask ratchet --init` writes the files from the current state;
it was run once when the rules came in, and a reviewer must see a reason before it runs again.

Lint counts are per file, not per place: fixing one `unwrap` and adding another in the same file passes. That is the
price of a ratchet that survives line moves. Size entries name the item, so they follow it through moves inside the
file; a renamed or moved item counts as new.

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

  Module rules have no ratchet: the tree must meet a rule when it is added. Phase 2 of the overhaul (`overhaul.md`)
  writes the rules for `ui`.
- The statics check reads the type as written and follows the structs and enums of the crate it names, through
  their fields: `OnceLock<Counters>` counts when `Counters` holds atomics. Types match by name, so two types of the
  same name in one crate count as one. A `thread_local!` is not seen (macro body).
- A metrics counter as a `static`: `static <file> <name>` in `xtask/data/architecture.txt`, with `<name>` as the
  gate prints it (`<function>::NAME` for a static inside a function). A listed static that no longer exists fails.
- Where `unsafe` may appear: `unsafe <path prefix>` in `xtask/data/architecture.txt` (a crate directory, a
  directory, or a file; `x` also covers `x.rs`). The list moves with the migration.
- A limit: the constants in `xtask/src/size/mod.rs` and the thresholds in `clippy.toml`. Lowering a limit needs
  `ratchet --init` for the new findings; say why in the commit.
