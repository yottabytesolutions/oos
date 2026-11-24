# Development: the gate

Workers run the fast gate and focused tests. The coordinator runs the default full gate once per integration batch:

```sh
cargo xtask check                  # default full gate, Docker-free
cargo xtask check --fast           # partial: formatting, syntax rules, architecture, agent rules
cargo xtask check --no-coverage    # partial: the full host gate without coverage
cargo xtask check --list           # inventory only, does not run or pass the gate
cargo xtask check --image          # default steps plus Docker/emulation/image suites
cargo xtask step clippy tests-debug # partial: only the named inventory ids (CI runs the gate as parallel jobs)
```

Until 0.2.0, repository CI is off and local evidence is authoritative. The coordinator runs
`--image` before unit rounds and releases. Before unit rounds or M-step acceptance, the
[delivery rule](process/delivery.md#publication-and-unit-rounds) also requires the M1 extended host
suites and logs in the task Status. Completion requires independent review and the full
gate on the final integrated candidate; `--fast`, `--no-coverage` and `--list` are partial feedback.

The full gate runs `perf-build` and `perf`. The fast gate skips both. Run
`cargo xtask step perf-build perf` to check the host performance guards separately.

Run from the repository root. Each executed step reports its time and failures, and a failed step
makes the command exit nonzero. A successful full run names its tier: `gate passed (default)`
or `gate passed (image)`. Hooks use the same command. Missing Docker cannot fail the default gate.

## Unit stage benchmark

`cargo xtask unit-bench` measures cold nanoseconds per record for each drawing stage by replaying recorded DMA
batches on a pinned CPU. On the unit, identical stage code showed a 2.90% difference in `deep / tile_writes`
over three rounds. Five-round repeats for that stage differed by +3.32% and -4.53% from their reference.
Use the [live acquisition matrix](#live-acquisition-matrix) for end-to-end acceptance.

Follow the [DMA fixture capture procedure](architecture/fixture-benchmark.md). Enable `acq.dump` for the
recording session, exercise each setup, and copy its completed batches from `/tmp/oos-batches` into a group
directory, for example `fixtures/short-1ch/…`. Disable the dump after recording. Keep the fixture directory,
reference and candidate JSON files outside the repository.

With the owner's release and a current [test lease](../tools/README.md#scope-lease), use trusted SSH and
SCPI port 5555 on the same unit. Run `ab` from the candidate checkout. Pass the reference checkout and use
the same fixtures and CPU for both builds. Renew the lease while the command runs.
The host uses system SSH, tar and SHA-256 tools (GNU tar and sha256sum on Linux, BSD tar and shasum on macOS).

```sh
cargo xtask unit-bench ab --unit root@example.invalid --fixtures /path/to/fixtures \
  --reference-checkout /path/to/reference-checkout --out /path/to/report.json
```

`ab` builds each checkout with its pinned Rust version. Each checkout must contain the fixture benchmark.
The reference build uses scratch space under the candidate's `target/`. The tool caches both binaries and
one copy of each fixture group on the unit. Within each group it runs reference, candidate, candidate,
reference and repeats that order, with at least five rounds per checkout. If acquisition is running,
`ab` pauses it once and restores it after the session. The JSON report contains both runs as `reference`
and `result`, and `ab` prints the comparison with the same exit codes as `compare`.

Separate `run` reports are valid for comparison within one session only. Run first from the reference
checkout and then from the candidate checkout with the same unit, CPU and fixtures. In two comparisons
with unchanged reference code on the same unit and boot about six hours later, `place` rose in nearly
every group. The two `deep` results rose 8.6% and 10.8%, `dense-1ch-1us` rose 4.4% and 7.8%, and
`dense-4ch-10us` rose 6.6% and 4.9%. Most remaining changes were between 0% and 4%.

```sh
cargo xtask unit-bench run --unit root@example.invalid --fixtures /path/to/fixtures --out /path/to/reference.json
cargo xtask unit-bench run --unit root@example.invalid --fixtures /path/to/fixtures --out /path/to/result.json
cargo xtask unit-bench compare --reference /path/to/reference.json --result /path/to/result.json
```

The run cross-builds the ignored release test, verifies its test listing and caches the executable and
fixture groups under `/tmp/oos-bench/` by SHA-256. By default it runs all groups for five rounds on CPU 2.
That adds about 67% to replay time compared with three rounds. Build and initial transfer costs are fixed per run.
Both `run` and `ab` accept repeatable `--group`, `--rounds` and `--cpu` to select a subset or change sampling. The report records the
checkout commit, executable SHA-256, the CPU number, the kernel's MIDR and a content SHA-256 for each fixture
group. The group hash covers sorted relative file names and file contents. Keep fixtures fixed during a run.
For `run`, use a stopped scope or `--stop` to pause acquisition before uploads and rounds. When either command
pauses acquisition, it sends `:RUN` on completion or error and checks that acquisition resumed. If restoration
fails, it reconnects and retries once. A failed retry reports the recovery command.
Process termination by signal, including Ctrl+C or SIGKILL, can leave the scope stopped. Before pausing the
scope, the command prints the recovery command below. Set `SCPI_HOST` to the unit's SCPI hostname first.
The SSH child runs in a separate process group and can continue after Ctrl+C. A benchmark round on the unit
has a 120-second remote timeout. Once the child has finished, remove the interrupted run's local scratch
directory with `rm -r -- target/unit-bench-PID-SUFFIX`, using the actual PID and suffix in the directory name.
An interrupted run can also leave an output staging file named `<output>.PID.tmp` next to the requested report.

```sh
printf ':RUN\n' | nc -w 2 "$SCPI_HOST" 5555
```

To profile the software Average worker with an existing plain fixture, run with
`--average-16 --group short-1ch-5ns`. The report includes `average_accumulate` along with decode,
trigger and resample. Use fixtures captured as raw samples at either sparse or dense time bases.

The report pools cold per-record timings across fixtures and rounds and gives the range of each stage's
round medians. It counts non-null samples once per record per round, and counts a round when it has at least
one timing for the stage. Stages with zero measured samples in both reports appear as `unmeasured`.
The fixture benchmark reads replay timings, independently of the SCPI acquisition and loss counters.

Each measured stage needs at least five measured rounds and ten samples in each report.
A median change must exceed both the tolerance (default 2%) of the reference stage median and
the sum of the two round-median ranges to count as `REGRESSION` (increase) or `faster` (decrease),
and a component change must also exceed the tolerance times the reference `place` median.
The component floor exists because a stage worth 0.2 % of placement was flagged as a regression between
two builds with identical stage code: three round medians understated its spread.
Changes within either shared threshold are `same`, and changes that clear both but fall at or
below the component floor are `within limits`.

Both `compare` and `ab` show the median of the percentage changes in `place` and the number of groups
with positive and negative changes. A median slowdown above the tolerance is a regression even if each
individual change is inside the round-median ranges. Sampling requirements also apply to the run verdict.

Two archived five-round runs of the same binary on CPU 2 on 2026-11-22 show the following
placement variation. Each spread is the round-median range divided by that run's pooled median.

| Group | Median change | Reference spread | Result spread |
| --- | ---: | ---: | ---: |
| `deep` | +3.12% | 2.38% | 4.43% |
| `dense-1ch-10us` | +0.97% | 2.12% | 3.06% |
| `dense-1ch-1us` | -5.22% | 7.83% | 10.71% |
| `dense-2ch-1us` | +1.04% | 4.49% | 4.65% |
| `dense-4ch-10us` | -0.28% | 3.07% | 5.74% |
| `dense-4ch-1us` | -0.66% | 2.32% | 3.84% |
| `dots` | -1.84% | 9.18% | 1.87% |
| `hires-1us` | -0.04% | 0.59% | 1.09% |
| `hires-200ns` | -0.05% | 0.79% | 0.79% |
| `measurement` | +1.39% | 2.66% | 2.62% |
| `peak` | +1.30% | 6.86% | 5.02% |
| `short-1ch-200ns` | +0.31% | 1.86% | 3.10% |
| `short-1ch-5ns` | -0.56% | 4.00% | 2.95% |
| `short-4ch-50ns` | +0.29% | 2.55% | 0.60% |
| `zoom` | -0.14% | 2.69% | 2.78% |

The two `dense-1ch-1us` round-median ranges overlap, with both endpoints lower in the result.
Archived reports retain the extrema and pooled medians. A cache or frequency shift that persists
through a run could move all rounds together without increasing its range.

The comparison exits 1 for a regression when sampling is sufficient. If a stage reports `insufficient data`,
the comparison exits 2 and shows the required rounds at the observed sampling rate. Rerun older reports
that lack sampling counts or group hashes. Other exit 2 errors include missing groups or stages, one-sided
measurements and mismatches in CPU, MIDR or fixture SHA-256. Use the same repeatable group selection to make
a subset reference.

## Live acquisition matrix

`cargo xtask unit-matrix` measures the installed build through SCPI. Use it for end-to-end acquisition
acceptance. Use [unit-bench](#unit-stage-benchmark) to isolate the cost of a drawing stage by replaying the
same DMA fixtures. On 2026-11-22, code layout alone moved live records per second by up to 5% between builds.

With the owner's release and a current [test lease](../tools/README.md#scope-lease), save the existing
setup to an available setup slot and stop other clients from changing the unit. Run the same groups on
the incumbent and candidate builds. Keep the signal and wiring fixed, with the trigger on CH2. The tool
uses SCPI port 5555 by default and needs the fresh-picture counters from a build with
`private-picture-counts`. Renew the lease while it runs.

```sh
cargo xtask unit-matrix run --scpi example.invalid --out /path/to/reference.json
cargo xtask unit-matrix run --scpi example.invalid --out /path/to/result.json
cargo xtask unit-matrix compare --reference /path/to/reference.json --result /path/to/result.json
cargo xtask unit-matrix sustained --scpi example.invalid --out /path/to/sustained.json --group dense-4ch-1us --minutes 10
```

The [matrix data](../xtask/data/unit-matrix.json) defines the base setup, group order and overrides, and
the additional setup keys the tool snapshots and verifies. Its `bench_setup` section sets the analog
input scales, offsets, coupling and trigger thresholds for the reference setup. The saved controls give
these display centers and trigger requirements. The original signal shapes and frequencies are unrecorded.

| Input | Signal for the checked-in settings |
| --- | --- |
| CH1 | DC-coupled input centered at 1.50 V, with excursions that fit the 10 mV/div view. |
| CH2 | DC-coupled input near 4.90 V, with repeatable rising edges across 4.93 V. This channel triggers every group. Its effective scale is 225 mV/div. |
| CH3 | DC-coupled input centered near 1.64 V, with excursions that fit the 1 V/div view. |
| CH4 | DC-coupled input centered near 0 V, with excursions that fit the 10 mV/div view. |

To use different signals, extract the `bench_setup` object as a JSON file, keep its keys and edit its
string values. Pass `--bench /path/to/bench.json` to both `run` and `sustained` as needed. Keep the CH2
trigger signal, wiring and transition rate fixed between reference and candidate runs. Reports identify
the configured bench by a SHA-256 digest of its settings, computed with `shasum` on macOS or `sha256sum`
on Linux.

`run` defaults to a five-second warm-up and three five-second windows per group. The two drift controls
default to one window each. Select groups with repeatable `--group` flags or override timing with
`--warmup`, `--window`, or `--windows`. Sustained mode warms up for ten seconds and prints each one-minute
window as it completes, including its late flips per minute based on measured elapsed time.
SCPI commands run with zero pacing by default. `--pace-ms` sets a minimum gap
after a write.

The report identifies the firmware build and gives per-window and pooled rates, counter deltas,
changed fallback causes and acquisition read-backs. Each query takes timestamps immediately before the
write and after the reply line arrives. The lower elapsed bound is the second write minus the first reply.
The upper bound is the second reply minus the first write. Fresh pictures/s uses both bounds. The
`shown_records_per_s`, `first_records_per_s` and `read_per_s` rates use the midpoint. Decreasing cumulative
counters invalidate the window with `counter reset` and fail the group. Sampling continues with the next group.
The per-window `records.deposited` delta can be negative when previously pending deposits are rejected.
Older reports use the raw worker placement and direct draw total, so raw deposit deltas across the two
accounting methods have different meanings. The `compare` metrics use first-use, fresh-picture and flip
counters. Sampling and comparison require `present.v2` target-refresh counters in every window.
Beside each late percentage, `compare` lists the distinct `flip.target_refreshes` values sampled
at the ends of the group's windows. A dash indicates a report without target values for that group.
A missing `watchdog ok` aborts sampling. The tool restores setup in two passes with offsets last and
verifies each saved key and RUN/STOP once. A snapshot value resembling an address or hostname causes
the tool to skip that key and print its name. After three consecutive SCPI transport failures,
restoration stops and reports the verified and unverified key names. The report retains available
windows when a group fails or the run aborts. Any group error, abort or restore failure makes `run`
or `sustained` exit 2.

Process signals such as Ctrl+C and SIGKILL skip automatic restoration. Recall the saved setup and use
the RUN/STOP setting in the recovery hint printed before the first change. The current
SCPI setup interface cannot remove a configured decoder slot. The command checks that the initial
decoder selection and configured slots are compatible with the archived setup before changing settings.
Finish any armed SINGLE capture before running the matrix.

`compare` requires the same bench setup, requested window length and window count per group. It reports
first-use records/s, midpoint fresh pictures/s, late flips per minute and the percentage of shown panel
pictures that missed their target refresh for each group. A group fails
if its first-use rate falls below the floor (95% of the reference by default) or it loses more than one
fresh picture per second. It also fails if more than 1% of shown pictures in the result group miss
their target refresh. The absolute limit is `LATE_FLIP_LIMIT_DIVISOR = 100` because on-time groups
should have near-zero misses. The limit is provisional until the first unit runs. It also applies
to four-channel groups with known target misses. Repeat a failing group, then investigate.

The run summary gives the total target misses and shown pictures for each side. Each group's late percentage
uses the totals from all its windows. Exit 1 means a group failed. Exit 2 means the reports are incompatible
or lack complete measurements.

## Setup

Install Git, rustup and Python >=3.12 as `python3` on macOS arm64 or Linux x86-64.
Other Linux and macOS hosts are supported but not verified. The BSDs are best effort: not guaranteed, but reported problems get fixed; on Windows use WSL 2 or a Linux VM (ADR 0010).
Doctor accepts newer Python versions and reports the interpreter actually used.
The default gate needs the scripts' declared shells (including `mksh` for Android GEL syntax),
coverage tools and the pinned Rust host/ARM toolchain. Syntax checking does not run the scripts.
HAL's boot card DTB tests also require `fdtget` on `PATH` for independent property readback.
Install `device-tree-compiler` on Debian/Ubuntu or `dtc` with Homebrew on macOS.
Only the explicit `--image` tier needs Docker with arm64 execution, QEMU, kernel source and the
locked GPU/simulation/image profiles. Prepare it with `tools/bootstrap.sh --image`.
Missing prerequisites fail in the tier that requires them.

Host scripts are POSIX `sh` and run on Linux, the BSDs and macOS. Where the systems differ (SHA-256 tools, file
sizes) they source `tools/lib/portable.sh`. `python3 -I tools/ci/host_scripts.py check` runs shellcheck as `sh` on
every script that runs on a host; the PR fast job runs it. Scripts that run only on the scope or inside a Linux
container are not host scripts (`SCOPE` in that file).

Then run:

```sh
# Manual prerequisite if rustup cannot start xtask or bootstrap:
rustup toolchain install 1.99.1 --profile minimal --component rustfmt,clippy,llvm-tools-preview --target aarch64-unknown-linux-musl
tools/bootstrap.sh
# Explicit optional container preparation:
tools/bootstrap.sh --gpu
tools/bootstrap.sh --sim
tools/bootstrap.sh --image
cargo xtask doctor
cargo xtask doctor --image
```

`rust-toolchain.toml` is authoritative for Rust 1.99.1, components and musl std. `tools/build-tools.json` requires
Python >=3.12 and pins cargo-llvm-cov 0.9.1 and cargo-nextest 0.9.151; bootstrap installs both with `--version =X --locked` and fetches
locked Cargo dependencies. The numbered distribution still needs a real setup run on this macOS arm64 host. Linux x86-64 runner
validation is unavailable here and explicitly untested, without blocking this release scope.
Do not replace a missing distribution/component with floating stable. An explicit override can defeat the file;
doctor rejects it even if that compiler prints the same release number.

Doctor installs nothing, pulls no images, changes no hooks, reads no keys and accesses no hardware. It reports
all independent base failures, expected/actual values and setup commands without printing private paths or tool
stderr. The default checks Rust/Cargo and override identity, components, musl std, matching rustfmt/clippy,
coverage and sysroot LLVM tools, the sysroot host `rust-lld` (no PATH lld required), Python version/stdlib,
Git/base ref, hooks, lockfile and writable isolated output. A missing or mismatched
`core.hooksPath` reports a warning without failing doctor (task 0025). Linker presence does not
prove release linking (0027).
`--gpu` adds Docker and locked software EGL/GLES execution on the Docker host platform;
`--sim` adds initramfs and SSH/firewall images, NET_ADMIN/nftables/IPv6 and host
`qemu-system-aarch64`. QEMU is checked only in the sim profile. `--image` selects all profiles
plus the compiler/rootfs/simulation roles and checked kernel source. Unselected profiles are
reported as not checked. Temporary probes remove
their containers with `--rm`, use no network and keep output scratch private. Missing selected prerequisites fail.

The default gate runs `doctor-base` without Docker, in addition to committed pin/lock syntax
alongside formatting. The image tier adds the GPU/simulation doctor profiles before their consumers.
Fast checks preserve G1 and task 0020 dispatch and need Python with `tomllib`.
The `protected-storage` fast check rejects raw storage-write primitives outside the checked writer in
`hal/src/update/card/writer.rs` (ADR 0011). Its explicit `dd` exceptions are regular host images, not devices.

Install Docker and QEMU separately, without using bootstrap as an OS package installer. On x86-64, configure arm64
emulation manually using the distribution's QEMU/binfmt setup.
Pinning the binfmt installer identity belongs to task 0033 (0.3.0). Neither doctor nor bootstrap executes
registration, sudo or a privileged container.

### Container bases, recipes and provenance

Each Dockerfile in `tools/build-env/` pins Ubuntu 26.04 or Alpine 3.22 directly by its multi-arch index digest,
resolved by the coordinator using `docker buildx imagetools inspect` on 2026-11-15.
`tools/container-images.json` maps each index to its arm64 and amd64 manifest digests and records each role's
recipe SHA-256 and package request list. Kernel amd64
additionally requests the cross compiler packages.

`tools/build-env.sh prepare ROLE` selects the platform manifest for the recipe's pinned index, explicitly pulls it
into the local Docker store, then builds from that manifest digest and the requested packages. Distinct manifest
references keep both architectures available for local verification in classic and multi-platform image stores.
Each `rootfs-<tool>` role requests only that builder's
packages. Simulation package scripts run normally; only the linux-lts installation skips scripts.
`resolve ROLE` checks local base digest/layer ancestry, platform and recipe/request contract label, returning
an immutable local image ID. Local tags are aliases only. Missing package version locks never reject setup:
apt/apk resolve the requested packages from their repositories. A cached role keeps its installed packages
until explicitly rebuilt; pinning the base and recipe does not freeze package versions or reproduce a payload.

Builders record the actual image's full installed package list, sorted as `name=version`, beside their outputs:
`kernel/out/base/build-packages.txt`, the GPU bundle's sibling `build-packages.txt`, GPU comparison output's
`build-packages.txt`, image output's `initramfs.packages.txt`, hardware helper's `oos-hw.packages.txt`, and
`rootfs/out/<artifact>.packages.txt` (`wlan-tools.packages.txt` covers both WLAN binaries).
The shared command is `tools/build-env.sh provenance ROLE --image sha256:ID --output FILE`.
These lists are informational provenance, with no expected-version comparison. They describe the builder image,
including transitive/base packages, rather than the packages in the assembled scope filesystem.

Task 0033 (0.3.0) owns complete per-role package manifest locks and the binfmt installer identity, together with
retained package sources, source timestamps and end-to-end payload reproducibility. Real builder runs on this
macOS arm64 host remain untested in the sandbox; Linux x86-64 validation is also untested and is not a hold here.

- Hooks, once per clone: `git config core.hooksPath .githooks`. pre-commit and pre-push run `check --fast`. The
  full gate runs in CI as the required checks (ADR 0005); run it locally for an early signal.
- The ratchet files are compared with the base branch `main` (or `origin/main`): the clone needs it
  (`git fetch origin main`).
- Every cargo call that resolves dependencies uses `--locked` (the `cargo xtask` alias, clippy, tests, metadata,
  coverage and `tools/perf-guard.sh`): a change of a dependency comes with its `Cargo.lock` change.

## Executable inventory and completion evidence

`xtask/src/inventory.rs` is the ordered executable registry. `cargo xtask check --list` prints its
stable IDs, commands, prerequisites and selection. The seven G1 fast steps retain their order,
followed by task 0020's agent rules, the test timing, the links and the text rules. Static tool/container pin
checks remain part of `fmt`.
The existing workspace registry must still cover every Cargo member. Ratchets, planned moves
and the coverage margin of 0.5 percentage points are unchanged.

| ID | Command or behavior | Prerequisite | Selection |
|---|---|---|---|
| `fmt` | `cargo fmt --all --check` and validate static pins | pinned tools | fast and full |
| `size` | syntax and move-aware size ratchets | workspace metadata | fast and full |
| `architecture` | workspace completeness and crate/module rules | Cargo.lock | fast and full |
| `mutable-statics` | syntax and per-crate ratchets | sources | fast and full |
| `unsafe` | syntax and per-crate ratchets | sources | fast and full |
| `method-exceptions` | disallowed-method exceptions | sources | fast and full |
| `known-failures` | known failures and open tasks | sources | fast and full |
| `agent-rules` | exact loaders and Git inventory | root AGENTS.md | fast and full |
| `test-time` | test sleep/deadline syntax and nonincreasing ratchet | sources | fast and full |
| `links` | links in README.md, SECURITY.md, `wiki/` and `docs/` name existing paths, wiki pages and headings | Git | fast and full |
| `text` | File rules over tracked and new files: line ends, final newline, SPDX, indentation, licence copies; ADR numbers and paths resolve | Git | fast and full |
| `doctor-base` | base doctor probes, including active toolchain identity | bootstrap; no Docker | full |
| `doctor` | `--gpu --sim` doctor profiles | base doctor, bootstrap, Docker | `--image` |
| `clippy` | host and ARM clippy per crate | musl std and clippy | full |
| `plan-matrix` | host `oos-fpgad` build; 65 acquisition plans against `xtask/data/plan-matrix.txt` | pinned toolchain, Python, `diff`; synthetic calibration; no local OOS defaults | full, Rust tier |
| `decoders` | `cargo check --all-targets --no-default-features` in `ui`: core only, then `--features dec-<name>` for each optional decoder alone | pinned toolchain | full |
| `tests-debug` | Rust tests per crate | pinned toolchain | full |
| `tests-release` | release Rust tests per crate | pinned toolchain | full |
| `perf-build` | compile the performance guards without running them | pinned Rust | full |
| `perf` | `sh tools/perf-guard.sh` | toolchain | full |
| `coverage` | `cargo llvm-cov` per crate | llvm-tools, cargo-llvm-cov | full; omitted only by partial mode |
| `script-syntax` | `python3 -I tools/script_syntax.py` | Git, Python, declared shells | full |
| `python-tests` | `python3 -I tools/python_tests.py`; isolated unittest discovery in tools/tests, tools/privacy and ui/tools, with count and skip checks | Git, Python; cargo-dependent tests skip without cargo or with `OOS_PYTHON_NO_RUST=1` in CI | full |
| `arm64-release-link` | receipted release build with `--bins -p oos-ui -p oos-fpgad --target aarch64-unknown-linux-musl` | musl std, rust-lld | `--image` |
| `gpu-host` | `sh gpu/test-host.sh` | ARM link and gpu-test profile | `--image` |
| `ssh-sim` | prepare initramfs/rootfs and nft/Dropbear/SFTP; `sh tools/ssh-sim.sh` | simulation profile | `--image` |
| `firewall-sim` | prepare initramfs/rootfs and nft; `sh tools/fw-sim.sh` | simulation profile | `--image` |
| `image-required` | `python3 -I tools/image_required.py` | authoritative Git base | `--image` |
| `image-suite` | `python3 -I tools/image_suite.py` | checksum-pinned kernel source, image profile, QEMU | `--image` |

Script discovery uses Git's NUL-separated tracked and nonignored candidate paths, recursively,
including extensionless shebangs, hooks and rootfs entry scripts. Python is compiled without
execution, with bytecode under the selected target. Shell fragments have explicit interpreter
mappings; executable C sources and the kernel's recorded Git answers are classified as data.
Unknown executable scripts fail. Python selections must be nonzero, their executed counts must
match discovery, and every skip reason is reported with the skipped count. Git-listed `test_*.py`
files outside the declared discovery roots fail the check. Host socket cases use synthetic fixtures and
the freshly receipted UI. No default-target fallback is accepted.

The ARM link verifies all three Rust executables (`oos-ui`, `oos-fpgad`, `oos-update`), their receipts,
and static AArch64 ELF headers. GPU comparisons require exactly the pictures named by every
`ui/tests/*.txt` script on both renderers, preserve differences under the selected target, and
check all three monitor modes. SSH/firewall use a prepared builder's BusyBox, without a kernel build.
Missing tools, fixtures, receipts or outputs fail. Missing Docker fails the image tier; the default
gate never probes Docker or the kernel source.

Every check run retains complete captured command output and step results in a unique
`check-logs/` directory under the selected `CARGO_TARGET_DIR`. The report shows failed, missing
prerequisite and not-selected checks. Exit status decides success, including QEMU and fixture
commands that print PASS before failing. Keep these logs with review evidence.

The image suite checks the pinned kernel source, builds `kernel/build-base.sh`, assembles with
`boot/build-dev-image.sh`, signs with `OOS_SIGNING_KEY` selected explicitly using `boot/sign-dev-image.sh`, and packages
with `python3 -I boot/build-gel.py`. It validates boot sections/ID, DTB, cpio contents, checksums,
receipts, update signature/payload and GEL contents, including the shipped `oos-hw` and GPU bundle.
It runs unit-DTB, GEL, USB and WLAN simulations. It uses synthetic inputs and worktree-local
builder caches; it performs no install, device access or private signing-key discovery. Supply the project release
signing key through `OOS_SIGNING_KEY` for this suite. CI publishes unsigned images for offline release signing.

The `image-required` ID is retained in the image tier as advisory change classification. It compares
with the protected `main` merge base plus working-tree inputs and examines both rename paths
and deletions. Missing bases and unclassified inputs fail closed in that tier. It never runs in
the default gate or changes that gate's completion result. The coordinator runs `--image` before
unit rounds and releases regardless of classification. Workflow files retain their base versions;
no CI aggregate or matrix result is required while repository CI is off.

The gate runs its checks at the same time, each as soon as its prerequisites passed, and prints each result when it
ends; the closing summary keeps the inventory order. Formatting uses one workspace-wide Cargo invocation, including
non-default members. The size, statics, unsafe, method-exception and known-failure checks share one source measurement
per gate run, including any parse error. A later run reads fresh sources. `perf-build` prepares the guards in the
selected target: HAL shares `test-release` with the release tests; UI retains the shipping `release` profile.
Cargo serializes builds that share its lock; timing guards still run alone after compilation ends.
Concurrent debug/release/coverage runners divide the host's cores between them; `NEXTEST_TEST_THREADS` overrides
that default. Single-step CI jobs retain nextest's defaults. The perf guards and the image checks run afterwards, one at a
time (`inventory::alone`): timing ratios and emulator time limits need a quiet machine. On a warm cache with
cargo-nextest, the default gate takes about 5 minutes on a 16-core laptop, bound by the golden pictures and coverage
of `ui`; the fast gate takes a few seconds. The image tier can take hours on cold builders or emulated hosts. Retain
actual timings and exit codes with coordinator evidence.

### Register plan baseline

`cargo xtask plan-matrix` runs the same check as `cargo xtask step plan-matrix`: build the host `oos-fpgad`,
generate the 65 cases from `tools/plan-matrix.sh`, and compare their complete output with the committed
`xtask/data/plan-matrix.txt`. A mismatch fails with a unified diff. An intended register change uses
`cargo xtask plan-matrix --update`; review and explain that baseline diff in the same pull request.
Missing baselines fail too; ordinary checks never rewrite them.

The baseline uses standard host defaults and the existing synthetic calibration generator. The step overrides `OOS_CAL_DIR` and
requires a host without `/oos/cal`, `/oos/limits.conf` or `/etc/oos/limits.conf`, because the CLI also searches
those locations. It prints plans only and accesses no hardware. It stays outside `--fast` because building
and running the CLI can take more than a second. The step reports its measured duration on every run.
`tools/plan-matrix.sh` remains for the calibrated comparisons used by the device deployment and test tools.
Host plan equality does not establish timing or calibrated behaviour on a unit (ADR 0008).

## Test time and completion

[ADR 0009](adr/0009-test-time.md) requires tests to advance elapsed time explicitly and await
worker completion events. Host perf guards check work counts or interleaved CPU-cost ratios. A timeout in
ordinary tests is only a named hang guard of at least 60 seconds; the shared host helper uses 120 seconds.

The `test-time` fast step follows every Cargo target and test module, including compound test-only cfgs
and cfg(test) statements or expressions inside production functions. It flags sleeps (including renamed
imports and macro bodies), clock comparisons inside or outside loops, bound elapsed/now values, real duration
assertions, and timeout calls shorter than the shared 120-second hang guard. Test-only clock reads inside
production behavior branches are also rejected. Tests of elapsed behavior inject their clock at construction
or use the same implementation through an explicit clock boundary.

Only named functions implementing hang guards, ignored manual observation tools and ignored performance
probes are exempt, in `xtask/src/test_time.rs`. Timestamp provenance checks compare against the feed origin,
without real duration budgets. Production and test builds use the same buffered worker receiver and the same
channel capacity. Test fixtures await the completion, retaining its value for the normal poll.

`xtask/data/<crate>/test-time.txt` is a store kind alongside the other ratchets. Its keys use
`time <file> <function> <finding>`. `cargo xtask ratchet` tightens it; `--move` applies `moves.txt` to it.
The protected-base comparison rejects additions or increases. Do not add new entries. The check is syntactic:
it does not infer time through arbitrary helper APIs or every possible macro DSL. Review must check those
boundaries too. Real duration budgets run only in ignored release `perf_guard` cases with `OOS_PERF_TESTS=1`.
Task 0088 records the audit and validation limits.

## Parser fuzzing

Install `cargo install cargo-fuzz --version 0.13.1 --locked`, then run from `fuzz/`:
`rustup toolchain install` (the pinned nightly) and
`cargo fuzz run scpi -- -max_total_time=60`. Replace `scpi` with `isf`, `wfm`, `ag`,
`setup`, `update` or `http`; `cargo fuzz list` lists them all.
The excluded fuzz crate uses small synthetic fixture seeds and in-memory parser
entry points. A crash is saved in `fuzz/artifacts/<target>/`; reproduce it with
`cargo fuzz run <target> artifacts/<target>/<crash-file>` and add a regression test.

## CI

GitHub-hosted runners run the inventory as parallel jobs, each one `cargo xtask step <id>...` (ADR 0005):

- `.github/workflows/pr.yml` on every pull request and merge-queue group: the required checks `fast`, `clippy`,
  `test` (debug and release), `python` and `deny`. `tools/ci/changes.py` lets a job skip its build when nothing it
  depends on changed: documents, `.github/` (except `pr.yml` and files a Rust test includes) and `tools/ci/` skip
  clippy, the tests and cargo-deny. A merge-queue group whose tree equals its PR head skips the checks that passed
  on that head. Skipped jobs still report success under their required name.
- `pr.yml` runs the tests in three parallel shards per profile (`OOS_TEST_PARTITION=K/N`). The last shard
  runs the UI golden binary alone. The remaining shards exclude that binary and use nextest
  `--partition hash:K/(N-1)`. Unsharded runs and 1/1 run every test. Doctests run in shard 1.
  A shard's skipped count includes tests assigned to other shards. Pull requests run debug tests; the merge
  queue adds release tests. The required `test` check covers every shard.
- The release tests, locally and in CI, build with the Cargo profile `test-release`: the release optimisation level
  and no debug assertions, without a fat-LTO link per test binary (16 codegen units). HAL's perf guards share this
  profile: tests unwind, just as under the former `perf` profile. UI's perf guards and the scope
  build keep the shipping `release` profile. Workspace host dev/test builds use line-table debug info for file/line backtraces;
  dependencies outside the workspace have no debug info. The test profile optimizes SHA-2's compression rounds
  and the UI's shared renderer/layout code (levels 2 and 1 respectively), retaining debug assertions and overflow
  checks. Development and shipping profiles are unchanged.
  Full workspace variable/type debug info is available with `CARGO_PROFILE_DEV_DEBUG=2`.
- The test and coverage steps run each crate's tests with cargo-nextest (pinned in `tools/build-tools.json`, installed
  by bootstrap, checked by the doctor): every test in its own process, all binaries in parallel, and the doctests
  with `cargo test --doc` (`cargo llvm-cov nextest` for coverage).
  `.config/nextest.toml` reports a test as slow after 5 s and ends it after 10 minutes (a hang guard); the step lists
  every test that ran 5 s or longer. A test that leaves a process holding its output fails. Tests kill a process
  group with `kill -KILL -- -<pgid>`: without `--`, the procps-ng `kill` of Ubuntu 24.04 signals the wrong group, or
  every process of the user (#23).
- Standalone decoder builds run in two parallel clippy matrix rows (`OOS_DECODER_PARTITION=K/N`). The
  core-only configuration and each optional feature alone are distributed round robin from the feature list.
  Unpartitioned gates still check every configuration. Host/ARM lint deduplication and ratchets remain in one
  row alongside register plans; the required `clippy` result waits for all decoder groups, lints and register plans. Nightly also separates
  the decoder matrix from lints.
- A push to `main` runs `pr.yml` as the only writer of the PR Rust caches (one per job kind); its test jobs build the
  tests without running them, because the merge queue already ran them on that tree. Rust caches retain workspace
  crates as well as dependencies, so those warm-up builds survive cleanup; PRs and queue groups only restore them.
- `.github/workflows/main.yml`, nightly and by hand: the whole default gate in parallel jobs, including the release
  tests, coverage and the base doctor, plus cargo-deny with the day's advisories and the image.
- The image job downloads the kernel base from the pre-release `kernel-base-<version>`
  (`tools/ci/kernel-base.sh download`); `tools/ci/kernel-base.sh publish` creates it for a new `kernel/base.sha256`.
- Each run ends with a `timings` job: job and step durations in the run summary (`tools/ci/run_summary.py`).

## Keeping build artifacts small

`cargo xtask check` and `step` default `CARGO_INCREMENTAL` to `0` for all children,
including shell/Python builders. Explicit developer settings take precedence.
The receipted builder and scripts sourcing `tools/output.sh` use the same default.
An interactive `cargo build` retains Cargo's default incremental behavior.
Heavy Rust-building CI jobs set `CARGO_INCREMENTAL=0`, including tests, coverage,
image and release jobs. PR and nightly fast jobs override it to `1` for xtask
and their children. Interactive `cargo xtask check --fast` retains Cargo's
incremental default for xtask; its children do not compile Rust. Explicit
developer settings still take precedence locally. Changing a profile invalidates
the relevant Cargo/cache fingerprints.

The gate uses one target tree for ordinary builds. `test-release` avoids fat LTO
for correctness tests and HAL timing guards; a separate `perf` profile no longer
duplicates those dependencies. UI timing guards retain fat LTO because their
ratios were qualified with it. Shipped binaries retain optimization level 3,
`lto=true`, `codegen-units=1`, and `panic=abort`. All musl AArch64 Cargo builds
inherit `target-cpu=cortex-a53`, static linking and `rust-lld` from
`.cargo/config.toml`, including `oos-fpgad`, `oos-ui` and `oos-update`. Record placement runs on
the four A53 cores, so the build is tuned for them. Against the same source built for `cortex-a72`, the
unit stage benchmark on an A53 core measured 4 to 9 % less time per record, and 42 % less on deep memory.
The same binary measured within noise on the A72 cores.

Coverage keeps its per-crate directories: concurrent `cargo-llvm-cov` runs clean
workspace artifacts, so sharing one would invalidate another run. After a valid
report, the gate runs `cargo llvm-cov clean --locked --profraw-only` in that
crate's selected target. Merged profdata, compiled binaries and captured JSON
remain available; failed test commands or malformed reports keep their raw profiles too.

For a long-lived checkout, Cargo's existing cleanup command removes stale
profiles, old feature/toolchain artifacts and obsolete nested coverage/perf trees:

```sh
build_target=$(python3 -I tools/output.py target) && cargo clean --locked --target-dir "$build_target"
```

Run this only while the checkout's builds and tests are idle. It resets the whole
selected target, including current artifacts, receipts and check logs, rather
than guessing which Cargo fingerprints are live. Save needed evidence elsewhere
first. The next build regenerates its artifacts and receipts. There is no age
heuristic that can silently discard an active build's files.

### Measured build costs (#152)

Measured on 2026-11-18 on the same macOS arm64 host (16 logical CPUs), Rust
1.99.1, using the default worktree-local `target/`. Cold means `cargo clean`
first, with dependencies already downloaded; warm immediately repeats the same
command. `CARGO_INCREMENTAL` was unset at entry for both versions, exercising
Cargo's former default versus the gate's new child default. The outer
`cargo xtask` invocation still uses Cargo's interactive default; that accounts
for the remaining incremental data. Explicit `CARGO_INCREMENTAL=0` also disables incremental compilation of xtask,
as in CI's heavy jobs.

Wall times include building xtask. Load is the one-minute host load at start/end;
the host was shared and heavily loaded, so these timings do not establish an
isolated speedup. The first baseline cold full run failed only
`support::group_kill_reaches_grandchildren_and_nothing_else` after 120 seconds;
the immediate baseline warm run passed. The after cold gate passed all 22
steps. The measured after warm run failed only the unchanged UI
`perf_guard_batch_core_learning`: projected A72 p99 9.292 ms exceeded its
9 ms bound. Its other 21 steps passed. These failures remain part of the
measurements; no test or gate setting was changed. A final verification with
`CARGO_INCREMENTAL=0` at entry took 258.5 s (load 6.5/24.2):
`gate FAILED in 257 s: tests-release`. Its unchanged
`warmed_ui_observer_allocation_census` observed three allocations instead of
zero; all other steps, including perf, passed. The complete after cold gate
above remains the passing full-gate run. The allocation failure remains outside
this build-only change; the load-sensitive learning guard is discussed below. The final
allocation failure passed when run alone with
`cargo test --locked --profile test-release -p oos-ui --test soak warmed_ui_observer_allocation_census`.

| Version / command | Cold wall (s) | Warm wall (s) | Cold load | Warm load | Cold target (MiB) | Warm target (MiB) |
|---|---:|---:|---|---|---:|---:|
| before, `check --fast` | 10.9 | 3.3 | 7.1/12.4 | 12.4/12.4 | 164.5 | 165.4 |
| before, `check` | 627.0 | 248.2 | 29.6/11.8 | 11.8/19.8 | 12740.9 | 12901.2 |
| after, `check --fast` | 22.0 | 5.3 | 77.5/107.6 | 107.6/111.6 | 133.2 | 134.1 |
| after, `check` | 635.2 | 231.9 | 114.3/47.9 | 47.9/24.1 | 2847.2 | 3019.4 |

Allocated disk after each full run (`du -sk`, MiB). The nested `deps`,
`incremental` and raw-profile rows overlap the profile totals; do not add them.
The total also includes retained command logs and script bytecode.

| Target subtree | Before cold | Before warm | After cold | After warm |
|---|---:|---:|---:|---:|
| `debug` | 6195.8 | 6175.0 | 1029.7 | 1029.7 |
| `test-release` | 329.8 | 328.4 | 329.1 | 329.1 |
| `perf-build/perf` | 35.3 | 35.3 | 0.0 | 0.0 |
| `perf-build/release` | 231.8 | 231.8 | 0.0 | 0.0 |
| `release` | 0.0 | 0.0 | 232.7 | 232.1 |
| `aarch64-unknown-linux-musl` | 423.0 | 422.5 | 31.8 | 31.7 |
| `xtask-cov` | 5337.3 | 5335.4 | 1036.1 | 1023.9 |
| `deps` | 2661.4 | 2654.0 | 2441.4 | 2418.2 |
| `incremental` | 7811.0 | 7778.1 | 54.5 | 54.5 |
| `profraw` | 2883.6 | 2883.6 | 0.0 | 0.0 |
| Total | 12740.9 | 12901.2 | 2847.2 | 3019.4 |

The former HAL `perf` and `test-release` settings produce the same test
optimization/unwinding behavior; sharing `test-release` eliminates that extra
build tree. UI timing guards still require the shipping `release` profile.
Per-crate coverage targets remain separate because their cleanup invalidates
workspace artifacts; successful raw profiles are discarded after merging.

Elapsed gate steps below include compilation, tests and Cargo lock waits. Steps
overlap, so their times are not additive or isolated compiler benchmarks.

| Profile / consumer | Before cold (s) | Before warm (s) | Initial after cold (s) | Initial after warm (s) | Corrected cold (s) | Corrected warm (s) |
|---|---:|---:|---:|---:|---:|---:|
| debug tests | 379.7 | 149.3 | 421.6 | 136.1 | 265.7 | 155.8 |
| test-release tests | 317.5 | 109.2 | 404.8 | 97.5 | 236.0 | 114.1 |
| HAL/UI guard builds | 445.9 | 1.2 | 475.6 | 1.4 | 321.4 | 1.0 |
| instrumented coverage | 350.5 | 192.0 | 401.8 | 171.4 | 243.9 | 191.9 |
| host + ARM debug Clippy | 507.1 | 11.5 | 503.4 | 13.1 | 346.7 | 13.1 |
| debug decoder feature matrix | 560.3 | 13.0 | 557.4 | 12.9 | 390.5 | 13.7 |

macOS already defaults to unpacked split debug info: default and explicit
`--config 'profile.dev.split-debuginfo="unpacked"'` builds reported every package
fresh (0.064 s and 0.049 s), reusing identical fingerprints without adding disk
usage. No redundant platform-specific override was added.

The separate ARM release-link step verified receipts and static AArch64 ELFs for
`oos-ui`, `oos-fpgad` and `oos-update`:

| ARM release build | Cold wall (s) | Warm wall (s) | Cold load | Warm load |
|---|---:|---:|---|---|
| before | 102.7 | 2.6 | 10.4/4.9 | 4.9/4.7 |
| after | 105.6 | 2.8 | 16.6/7.1 | 7.1/7.1 |

All three shipped executables were byte-identical before/after (SHA-256),
with the existing target flags and full release optimization unchanged.
No alternate release linker flags were introduced. Host builds only; no unit
installation, reboot or hardware run was performed.

### Regression checks (#158)

The initial fast-gate timings above compared very different host loads. Adjacent
reruns with the main and PR manifests, fresh targets and incremental xtask builds
showed no fast-gate slowdown from the lean profiles:

| Manifest / command | Cold wall (s) | Warm wall (s) | Cold load | Warm load |
|---|---:|---:|---|---|
| main, `check --fast` | 22.2 | 5.0 | 42.6/69.9 | 69.9/79.6 |
| PR, `check --fast` | 14.0 | 3.5 | 79.6/91.6 | 91.6/91.6 |
| corrected PR, `check --fast` | 7.9 | 2.1 | 40.7/36.1 | 36.1/36.1 |

Separate warm PR runs took 2.05 s with `CARGO_INCREMENTAL=0` and 1.98 s with
`CARGO_INCREMENTAL=1`. The former 10.9/3.3 s targets cannot be attributed to a
profile regression when main also exceeds them under load. The corrected run
meets both targets; different host loads prevent claiming an isolated speedup.
Fast CI jobs now
explicitly keep incremental compilation enabled; heavy builds remain lean.

The main learning guard was built with the main manifest and unchanged main
UI/HAL/core/DSP sources. It passed alone, then failed 2 of 5 adjacent loaded runs
while the baseline full gate was building. Its fixed projected-A72 assertion
reported 11.088 and 10.603 ms against 9 ms at host loads 167-169. This assertion
predated the PR; HAL's shared `test-release` artifacts did not cause it.
At that point the projected-A72 assertion still ran alongside best <= 1.0 and p99 CPU <= 1.5.
The acquisition sweep retains those cost ratios and makes the 9 ms A72 budget a measured unit target.
A fixed host-to-unit factor cannot account for CPU model, frequency and memory differences.
The fast-job environment regression test also failed before its override was added.

The unchanged PR passed the full gate using the existing target:
`gate passed (default): 22 steps in 480 s` (480.6 s wall, load 37.5/28.8).

The corrected full gate then ran cold in a fresh selected target and immediately
repeated warm, with `CARGO_INCREMENTAL=0`. The fast pair used `1`, as in fast CI.
All 22 steps passed both times, including perf, debug/release tests of every
workspace crate, coverage and Python tests. No ratchet counts decreased in the
changed files. The corrections add zero production Rust lines.

| Corrected command | Cold wall (s) | Warm wall (s) | Cold load | Warm load | Cold target (MiB) | Warm target (MiB) |
|---|---:|---:|---|---|---:|---:|
| `check --fast` | 7.9 | 2.1 | 40.7/36.1 | 36.1/36.1 | 132.2 | 133.1 |
| `check` | 454.3 | 250.1 | 36.1/10.1 | 10.1/19.8 | 2762.1 | 2955.5 |

The full gate printed `gate passed (default): 22 steps in 449 s` cold and
`gate passed (default): 22 steps in 250 s` warm. The lean target still occupies
2.89 GiB warm. These are host-only measurements; no hardware was accessed.
These cold/warm runs preceded review and used the ratio-only learning guard;
they do not validate the restored projected-A72 assertion.

## Privacy checks

Before committing, run `python3 tools/privacy/scan.py --staged` and
`python3 tools/privacy/scan.py --message target/COMMIT_MSG.txt`. The stdlib scanner checks the index and commit message;
the incoming-commit scanner can check additions and messages locally while Actions are disabled.
The Privacy workflow, when enabled, checks every incoming commit. Findings redact values.
Use documentation addresses and placeholders in examples, and configure device tools explicitly with `--host`
or `OOS_SCOPE` where supported. Never infer a real unit address from a checked-in example.

Keep private strings only in the ignored `local/privacy-literals.txt`, one exact value per line. Reviewed synthetic
exceptions key the exact path, rule and matched-value hash in `tools/privacy/allowlist.json`. See [the scanner instructions](../tools/privacy/README.md)
for commands, limitations, the policy format and the pending hook integration. The xtask hook point waits for
G1/task 0043; this check does not replace the full gate.

## Loading the shared agent rules

Root `AGENTS.md` is the single source of repository rules. The `agent rules` step runs after `known failures`
in both fast and full checks, including the hooks. It checks these complete loader files:

| Entry file | Permitted content |
|---|---|
| `CLAUDE.md` | the root rules pointer, optionally followed by the exact prose instruction to use the harness-provided `Claude-Session` trailer and session URL |
| `GEMINI.md` | the root rules pointer |
| `.github/copilot-instructions.md` | the root rules pointer through `../AGENTS.md` |
| `.cursor/rules/oos.mdc` | the root rules pointer through `../../AGENTS.md`, with exactly `description: OOS repository rules` and `alwaysApply: true` loader metadata |
| `.aider.conf.yml` | only `read:` followed by the single list item `  - AGENTS.md` |

The check compares complete templates, normalizing only CRLF and the presence of one final newline. Extra rules,
comments, loader keys and actual Claude session identifiers fail. Required files must be readable regular files;
symlinks in their paths fail. The relative Markdown links must resolve to the existing, nonempty root `AGENTS.md`.
Template mismatches report the first differing line without echoing file content. There is no allowance or ratchet
for this check.

Git supplies tracked and nonignored new paths with `ls-files -z --cached --others --exclude-standard`.
Additional recognized entry points fail until a reviewed inventory change supports them: `AGENTS.md`, `CLAUDE.md`,
`GEMINI.md`, `CLAUDE.local.md`, `.aider.conf.yml` and `.cursorrules` at any depth, files under any `.claude/`,
`.gemini/`, `.cursor/rules/`, `.github/prompts/`, `.github/agents/` or `.github/chatmodes/`, and Copilot
`copilot-instructions.md` files under `.github/` or `*.instructions.md` under `.github/instructions/`, including
nested directory layouts. Discovery is case-insensitive; supported entries retain their canonical spellings so
case aliases cannot introduce unchecked files on case-sensitive hosts. Tracked files remain checked when ignored
or deleted. Ignored personal settings are outside this inventory; the checker does not recognize every client's
present or future configuration.

For a client without automatic discovery, explicitly attach or read root `AGENTS.md` at session start. Aider uses
the checked `read` configuration; `/read AGENTS.md` also loads it explicitly. A passing entry-file check cannot
prove that a client loaded or obeyed the rules.

## The crates

One root workspace (`Cargo.toml`): `hal`, `dsp`, `core`, `ui`, `fpgad` and `xtask`, with one `Cargo.lock`, one `target/`
and the release profile of the scope binaries. `gpu` is C (`gpu/oos-gpu.c`), built in its own container by
`gpu/build.sh` and `gpu/test-host.sh`, so it is no member. The scope's binaries land in
`target/aarch64-unknown-linux-musl/release/`; the target flags (rust-lld, static, Cortex-A53) are in the root
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
| disallowed-method exceptions | syntax walk | `#[allow]`/`#[expect]` of `clippy::disallowed_methods` only as listed in `allow.txt` | an exception cannot spread unseen |
| known failures | syntax walk | `#[ignore = "known: task NNNN"]` only as listed in `known.txt`, each naming an open task | a failing test does not hide |
| agent rules | exact loader templates and Git path inventory | the five checked client entry files load root `AGENTS.md`; additional recognized entries fail | universal rules stay in one place |
| architecture | `cargo metadata`, syntax walk | `xtask/data/architecture.txt`: allowed crate dependencies; module rules inside a crate | the layers cannot erode by accident |
| clippy | `cargo clippy --all-targets`, for the host and for `aarch64-unknown-linux-musl` | the `[lints]` table of each crate and `clippy.toml`; the scope build compiles | correctness, idiom, size and complexity, also in Linux-only code |
| tests (debug, release) | `cargo nextest run`, `--cargo-profile test-release` | every test passes in both profiles, full counts | release-only bugs (overflow checks, timing) show |
| perf guards | `tools/perf-guard.sh` | the fork's work counts and CPU-cost ratios; its exit code decides | the fast paths stay fast |
| coverage | `cargo llvm-cov` per crate | line coverage per crate ≥ the floor in `xtask/data/coverage.txt` | coverage does not fall |

The limits have no exemptions (ADR 0006): a record over 12 fields groups its fields into sub-structs.

A function's size is its code lines from `fn` to the closing brace: lines with a token on them, so blank lines,
comments and doc comments do not count (ADR 0006). A file's size is its lines outside test code without comment lines,
doc comments included, so documentation never competes with code for space; blank lines count (ADR 0006). Test code
is exempt.
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

One file per crate and kind, `xtask/data/<crate>/<kind>.txt`, so branches on different crates do not touch the same
file:

- `size.txt`: `<size> <kind> <file or crate> <name>`, every item over a size limit.
- `clippy.txt`: `<count> <crate> <file> <lint>`, every lint finding.
- `statics.txt`: `1 static <file> <name>`, every mutable static that is not a metric.
- `unsafe.txt`: `<count> unsafe <file>`, `unsafe` outside the allowed paths.
- `allow.txt`: `<count> allow <file> clippy::disallowed_methods`, every `#[allow]` or `#[expect]` of the disallowed
  methods (also inside `cfg_attr` and as `#![allow]`), so an adapter-level exception cannot spread unseen.
- `known.txt`: `1 known <file> <test> <task>`, every test ignored as a known failure with
  `#[ignore = "known: task NNNN"]`. The task must be open (in `docs/tasks/`, not in `done/`); an `ignore` reason that
  starts with `known` but names no task fails. Every `#[ignore]` carries a reason, and a `flaky:` reason names its
  issue: `#[ignore = "flaky: #123 races the timer"]`. A bare `#[ignore]` or an untracked flaky test fails.

`xtask/data/coverage.txt` holds the coverage floors, `<floor> <crate>` in tenths of a percent, for all crates.

The gate fails when a listed entry grows, when an entry appears that is not listed (a new oversized function, a new
lint finding in a file), and when coverage falls more than 0.5 points below its floor: measurements move a little
between runs, and the pull request that raised coverage raises the floor (`--coverage`). For files touched since the
protected base, including local changes, an allowance above the current count fails as unused slack. This includes
findings that disappeared and deleted files. Untouched files keep their allowances. Planned moves compare the
allowance at the destination path. Write the improvement back:

```sh
cargo xtask ratchet               # lower sizes and counts to the current values, drop fixed entries
cargo xtask ratchet --coverage    # also raise the coverage floors to the current values
```

`ratchet` never raises a size or count and never adds an entry: a new finding is fixed, not listed. The pull request
that lowered a count runs `cargo xtask ratchet` and commits the result; nobody edits a ratchet file by hand
([ADR 0006](adr/0006-format-lints-and-ratchets.md)). Two pull requests that tighten the same line conflict: the second rebases and runs
`cargo xtask ratchet` again.
`cargo xtask ratchet --init` writes the files from the current state; it only works where the base has no gate.

There is no exception. The upstream rebaseline path is gone with the hard fork
([ADR 0001](adr/0001-hard-fork.md)); a cherry-picked upstream change passes the gate
like any other.

### Moves

A branch that moves code lists its moves in `xtask/data/moves.txt`, one `<old> -> <new>` per line:

- a key move: both sides are ratchet keys as the gate prints them, `fn ui/src/ui.rs Ui::step -> fn ui/src/step.rs
  Ui::step`;
- a file move: both sides are paths, `ui/src/model.rs -> core/src/model.rs`; it renames the path in every key and the
  crate word of the clippy keys.

The gate then accepts a listed entry under its new key if it is not larger than under the old one at the base. A move
that is not listed makes the entry new, and it fails. A move onto a key that already has an entry fails, and so do two
moves onto one key: merged entries would share the larger allowance. Moves apply in the order listed. A moved entry
may land in a new crate: it brings its allowance with it.

The branch runs `cargo xtask ratchet --move` and commits the result (ADR 0006): it renames the
entries in the ratchet files and keeps in `moves.txt` the moves that still rename an entry at the base. They are the
evidence the integration commit needs against a base without them. A move that names no entry changes nothing, so a
kept move is harmless; the next `ratchet` or `ratchet --move` whose base has the renamed entries drops it. A moved
entry that lands in no crate of `CRATES` fails before anything is written. Plain `cargo xtask ratchet` refuses while a
listed move still names an entry of the ratchet files. The base comparison applies the moves of the base and of the
branch to both sides.

### Base trust

The gate compares each ratchet file with its version at the merge base of `HEAD` and `main` (`origin/main`
in a CI checkout), or `HEAD^1` when `HEAD` is on `main` itself. An entry that is not there, a larger value, or a
removed or lowered coverage floor fails. Any git error fails. `--init` refuses a ratchet file the base already has.

- A ratchet file absent at the base is legal only while the base has no `xtask`: the one merge that brings the gate
  in. After it, a missing ratchet file fails.
- The required CI checks are authoritative (ADR 0005). Setting `CI` or `OOS_GATE_BASE` by hand, or moving a local
  `main` to pass a check, breaks the rules.
- On `main`, `HEAD^1` covers one commit. A push of several commits is not compared commit by commit: the merge
  queue runs the required checks on the pull request's commits before `main` fast-forwards.

Lint counts are per file, not per place: fixing one `unwrap` and adding another in the same file passes. That is the
price of a ratchet that survives line moves. Size entries name the item, so they follow it through moves inside the
file; a renamed or moved item counts as new unless `moves.txt` lists it.

## Production code size

Each release has less production code than the one before
([ADR 0007](adr/0007-less-production-code.md)). The counting specification is:

- Rust production lines contain a token in crates built for the scope (`target: true` in `CRATES`,
  `xtask/src/repo.rs`). Count this project's C in `gpu/` and `tools/hw/` per directory as well, excluding lines that
  contain only comments. Leave out the vendored Xilinx XDMA driver in `kernel/modules/xdma/`.
- Exclude blank lines, comments and doc comments. Exclude test items, fields, variants, statements, match arms and
  field values under `#[test]` or a `cfg` requiring `test`, with their attributes; files below a `#[cfg(test)]`
  module; and test, bench and example targets (`tests/`, `benches/`, `examples/`).
- Exclude files marked `@generated` in their first five lines. Goldens, fixtures and vendored data are not Rust
  modules and are not reached by the count.
- Report `xtask` (host-only Rust) and shell and Python scripts as tooling beside production code. Script lines count
  unless blank or a `#` comment; test directories and test files are excluded. Python docstrings count. Scope
  shell scripts in `rootfs/` are tooling too. Tooling is reported, not held to the release goal.
- For releases before the root workspace (through v0.1.13), `cargo xtask loc` reads each top-level crate as its own
  workspace and follows plain `#[path]`; this does not permit `#[path]` in the gate.

Generated tables ([ADR 0029](adr/0029-table-generation.md)) can reduce the count without
reducing complexity. Review the represented behaviour and reuse as well as the number. Tests and documentation must
not be deleted to shrink the count. File-size limits separately exclude comments, including doc comments, while
still counting blank lines.

```sh
cargo xtask loc                      # per crate and in total, as Markdown tables
cargo xtask loc --compare v0.2.0     # the change since a git ref, per crate
cargo xtask loc --json               # the same numbers for a script
```

The `fast` job puts the change against the pull request's base into its summary; it never fails. The release
workflow compares the tag with the previous `v*` tag, adds the table to the draft release notes, and fails when
production code grew without a `Code growth:` paragraph in the tag annotation or the release's `CHANGELOG.md`
section (`tools/ci/code_growth.py`). Review requires owner approval of that paragraph.

## Changing a rule

- A new crate: add it to `CRATES` in `xtask/src/repo.rs` and a `crate` line to `xtask/data/architecture.txt`. It has
  no ratchet files, so it starts clean, except for the entries a planned move brings into it. `ratchet --move` writes
  its files.
- A new dependency: extend its crate's line in `xtask/data/architecture.txt`. The gate fails until the line allows it,
  so the decision is visible in review.
- Module rules in `xtask/data/architecture.txt`, over module paths such as `board` or `board::dho`, and over paths
  into the crate's dependencies under the package name with `_` (`oos_hal::dho`). Every path in the code counts
  (`use` trees, calls, types) in every spelling that names the same item: `crate::`, `self::` and `super::` resolved
  against the file's module and any inline `mod`; a child module named from its parent (`dho::x` in `board`); names
  bound by `use` or `extern crate ... as`, in the module or, for a `use` inside a function, only in its block; glob
  imports (into a dependency always, into a crate module where the name exists); re-exports in any module of the
  crate, followed transitively (`pub use self::dho as hw;` in `board` makes `crate::board::hw` count as
  `board::dho`); `::oos_hal`; a Cargo rename (`hal = { package = "oos-hal" }` makes `hal::dho` count as
  `oos_hal::dho`); raw identifiers (`r#dho` is `dho`). Macro bodies are tokens, not paths: a `confine` rule also
  fails a file whose tokens outside test code spell the confined path (`board::dho`, `oos_hal::dho` or a renamed
  `hal::dho`) outside the allowed modules. Comments and string literals do not count. A module is inside a rule's module when its path starts with
  it segment-wise; for a file, the most specific `layer` rule applies. Test code is free: test targets, test files
  and `#[cfg(test)]` or `#[test]` items.
  - `forbid <crate dir> <from> <to>`: code under `<crate dir>/src/<from>.rs` or `src/<from>/` must not name
    `crate::<to>`.
  - `layer <crate dir> <module> -> <modules...>`: code of `<module>` may name only the listed crate modules and
    itself; nothing after `->` makes it a leaf. Dependency paths are not layer business.
  - `confine <crate dir> <path> -> <modules...>`: `<path>`, a crate module or a dependency path, may be named only from
    inside the listed modules (and a crate module also from inside itself). The board boundary is two of them: only
    `board::dho` uses `oos_hal::dho`, and nothing outside `board` names `board::dho` (`board/mod.rs` selects the
    port).

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
  A path relative to the current module (`dho::x` inside `board`, 2018 edition) is not resolved; nor is a
  dependency path behind a `use oos_hal::dho as d` rename.
- Impl spread counts the spelling of the self type (its last path segment): aliases split one type, and two types of
  the same name merge.
- Path dependencies are checked by package name only, not by source: a local fork of an allowed crate passes.
- Types imported from another workspace crate, and associated types, are not followed by the statics check.
- The base branch is the local `main` ref; the coordinator validates it while repository CI is off.
- pre-commit checks the working tree, not the staged index, and only the fast part. The done rule is the full gate
  on the final commit.

## Layout baseline procedure

For a behaviour-preserving layout change, run `ui/tools/layout-check.sh record` before editing and `check` after it.
Keep those before/after artifacts with review evidence. Continue this procedure until golden tests cover each
layout-check script and all three text sizes. The baseline follow-up in the migration plan owns the coverage
check and removal of this temporary procedure. Golden changes still need a reason and maintainer approval (ADR 0008).

## ADR reference check

The `text` step inventories existing tracked and new files. It checks duplicate record numbers and every numeric
ADR citation and ADR file path, including dotfiles, all source files, code comments and task history.
Retired category paths are rejected even without a Markdown extension. Backticked step IDs in ADR Check paragraphs
must exist in the executable check inventory. The flat record files are the source.
`links` additionally validates Markdown link anchors. Historical task citations use the current numbers.
