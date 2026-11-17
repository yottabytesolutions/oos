# Contributing to OOS

This tree is a hard fork of OOS with its own trunk, `main`, and its own rules
([ADR 0030](docs/adr/process/0030-this-tree-is-a-hard-fork-of-oos.md)). Contributions from people and from coding
agents are welcome under the structure below.

Reports, measurements and code are all welcome. The most useful thing right now is a test on hardware: OOS has run on
a DHO924S and a DHO804 so far. A report from a DHO802/DHO812, a DHO914/DHO924 without generator or an EDU model makes
that whole hardware variant verified ([docs/models.md](docs/models.md) 2.1).

## Ground rules

- **No vendor code or vendor data.** No firmware images, no code or binaries from any stock or modified firmware, no
  calibration files, no `vendor.bin`. What runs on the scope only depends on `libc`, and it stays that way unless
  there is a strong reason; a desktop-only build may use its platform's crates
  ([ADR 0021](docs/adr/process/0021-libc-only-on-the-scope-platform-crates-on-the-desktop.md)).
- **No vendor material in the texts either.** Code, comments, documents, commit messages, pull request descriptions
  and issues describe the hardware as behaviour, in your own words: no listings, copied tables or strings from the
  stock or a modified firmware, and no names of its functions, classes or files. What the vendor publishes for users
  is fine: the command names of the programming guide, the manuals, the data sheets of the parts
  ([ADR 0026](docs/adr/process/0026-contributions-are-signed-off-and-hold-no-vendor-material.md)).
- **Sign your commits off.** `git commit -s` adds a `Signed-off-by:` line with your name. With it you state the
  [Developer Certificate of Origin](https://developercertificate.org): the work is your own or you have the right to
  submit it, under the licence of the project. This holds for work your agent wrote too. Code or text from another
  project is named in the pull request with its source and licence.
- **Say how you know.** A statement about the hardware in `docs/` is "verified on the unit", or "assumed", or "not yet
  verified on hardware". Keep that honest: a wrong "verified" costs someone else days.
- **Be careful with other people's scopes.** OOS never writes the FPGA flash, `/rigol/data`, or the `uboot`, `trust` and
  `parameter` partitions. Changes that would need any of this will not be accepted without a very good case.
- **Keep the analog path unchanged unless that is the point.** `tools/plan-matrix.sh` prints the register sequences of a
  fixed set of acquisitions. Run it before and after a change and compare the output. Say in your pull request whether they differ.
- **No personal data in issues or commits:** no serial numbers, MAC addresses or licence keys.

## A new protocol decoder

A decoder comes as a protocol module: one file in `ui/src/protocols`, one line to register it, the decoder itself
usually in `oos-dsp`. [docs/protocol-modules.md](docs/protocol-modules.md) walks through it.

## New measurements

Measurements come as a measurement module: a file in `ui/src/measures` with their tiles and their computation, one
line to register it, the signal processing usually in `oos-dsp`. [docs/measurement-modules.md](docs/measurement-modules.md)
walks through it. An analyser page registers the same way ([docs/analyser-modules.md](docs/analyser-modules.md)).

## Quick start

Development runs on Linux, the BSDs and macOS. On Windows, work in WSL 2 or a Linux virtual machine: the gate, the
hooks and the scripts in `tools/` expect a POSIX host and run there as on any Linux computer
([ADR 0033](docs/adr/process/0033-development-platforms.md)). Project tooling written in Rust goes into `xtask`, small
helpers into `tools/` ([ADR 0031](docs/adr/process/0031-the-gate-is-cargo-xtask-check.md)).
In WSL, clone into the Linux file system (for example `~/oos`), not under `/mnt/c`: builds there are much slower,
and file modes and line ends do not survive the Windows file system.

```sh
git clone https://github.com/yottabytesolutions/oos && cd oos
tools/bootstrap.sh                 # installs the pinned toolchain from rust-toolchain.toml and the host tools
git config core.hooksPath .githooks
git config pull.rebase true
git config rebase.autoStash true
cargo xtask check --fast           # before every commit and push (the hooks run it)
cargo xtask check                  # the full gate, optional: CI runs it on every pull request
```

The gate is `cargo xtask check` ([ADR 0031](docs/adr/process/0031-the-gate-is-cargo-xtask-check.md)). The
[check inventory](docs/development.md#executable-inventory-and-completion-evidence) lists the steps and their
prerequisites (`cargo xtask check --list`); `cargo xtask step <name>` runs single steps. `cargo xtask check --image`
adds the Docker and emulation checks; the maintainers run it before unit rounds and releases. The hooks run
the fast tier; the full gate runs in CI as the required checks. Run it locally only when you want an early signal. A
change is done when the required checks passed on its final commit and the review accepted it.

Tests use synthetic inputs where possible. Tests that need private calibration files report their skips; no private
calibration directory is required for the gate. [docs/architecture/README.md](docs/architecture/README.md) is the
place to start reading.

## Branches, commits, pull requests

- One topic per branch and pull request; one issue per pull request, or per stack when the work spans several pull
  requests ([ADR 0026](docs/adr/process/0026-contributions-are-signed-off-and-hold-no-vendor-material.md)). Branch:
  `<type>/<ref>-<slug>`, `type` one of `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `build`, `chore`;
  `ref` the issue number or `t` plus the four-digit task id; `slug` lower case with `-`. Examples:
  `fix/123-trigger-holdoff`, `refactor/t0075-normal-trigger`. `main` is the only long-lived branch.
- Pull request title and every commit subject: Conventional Commits, `type(scope): summary`, at most 72 characters.
  Pull requests merge by rebase, so each commit lands on main as it is and its subject becomes a changelog line.
  Fold `fixup!`, `squash!` and WIP commits into their topic commit (`git rebase -i --autosquash`) before you mark
  the pull request ready; `pr-meta` rejects them.
- Every commit carries `Signed-off-by:` (`git commit -s`) matching the author (DCO, ADR 0026). Use your GitHub
  noreply address (`<id>+<login>@users.noreply.github.com` or `<login>@users.noreply.github.com`): other email
  addresses are rejected by the privacy check.
- Commits and pull request texts carry no tool or session details: no session links, no `Agent:` and no AI
  `Co-authored-by:` trailers ([ADR 0019](docs/adr/process/0019-one-style-for-text-files-and-commits.md)).
- Keep pull requests small: about 400 changed lines without goldens and fixtures. Work that needs more than one pull
  request goes as a stack ([below](#stacked-pull-requests)).
- Link the issue (`Closes #NNN`) or the task file in the pull request text. Fill in the template.
- History is linear. Rebase onto `main` (`git pull --rebase`) instead of merging `main` into your branch, and
  force-push your own branch with `--force-with-lease`. `main` is never force-pushed and only moves by fast-forward:
  pull requests enter through the merge queue by rebase, with your name and sign-off kept on every commit.

## Stacked pull requests

Work that spans more than one pull request is a stack: a chain of pull requests where the bottom one targets `main`
and each higher one targets the branch below it
([ADR 0034](docs/adr/process/0034-multi-pr-work-lands-as-a-stack.md),
[GitHub's documentation](https://docs.github.com/en/pull-requests/get-started/about-stacked-prs)). Each layer is one
topic, reviewable and green on its own, and keeps the branch scheme above; the layers share the `ref`. Lower layers
link the issue with `Part of #NNN`; only the top layer says `Closes #NNN`, so the issue closes with the last merge.

```sh
gh extension install github/gh-stack           # once; needs gh 2.90 and Git 2.36 or later
gh stack init refactor/42-split-bus            # bottom layer, based on main
git commit -s ...                              # the first topic
gh stack add test/42-bus-fakes                 # next layer on top of it
git commit -s ...
gh stack submit --auto                         # push every layer, open them as draft pull requests, one stack
gh pr edit <number> --title 'type(scope): summary'   # each layer: --auto titles a multi-commit layer by branch
gh stack view                                  # the layers, their pull requests and their state
gh stack down; git commit -s ...; gh stack rebase --upstack; gh stack push   # fix a lower layer
gh stack sync                                  # after main moved or a layer merged: rebase and push
gh pr ready <number>                           # each layer once it is reviewed: a draft cannot merge
gh stack merge <stack>                         # maintainer: queue the layers bottom up (stack number from the PR)
```

- Every layer runs every required check, as any pull request does. The ruleset of `main` applies to every layer of a
  stack based on `main`.
- A stack merges bottom up, through the merge queue, as rebase merges: every commit of a layer lands on `main` as it
  is. Before the last review, fold the fix-ups into their topic commits, so each commit is one signed-off
  Conventional Commit: `git commit -s --fixup=<topic commit>`, then `GIT_SEQUENCE_EDITOR=: git rebase -i --autosquash
  <base>` with the branch of the layer below (`main` for the bottom layer) as `<base>`, then
  `gh stack rebase --upstack` and `gh stack push`.
  When a layer merges, GitHub retargets the next one to `main` and rebases it. A layer removed or ejected from the
  queue takes every layer above it out of the queue too.
- Update a stack with `gh stack sync` or `gh stack rebase`, never by merging. Their pushes use `--force-with-lease`
  on the stack's own branches, which is allowed.
- A rebase gives the layers above a new head commit: comment `@codex review` on each of them afterwards, or their
  `codex-review` stays pending.
- Agents keep to [AGENTS.md](AGENTS.md#working-together): only the coordinator runs the commands that push (`submit`,
  `push`, `sync`, `link`), with the owner's go; only the maintainer runs `gh stack merge`.
- All layers live in this repository: GitHub does not support stacks across forks. Without push access, submit one
  layer at a time from your fork and open the next when the previous one merged, or ask the maintainer for push
  access or to recreate the stack from your fork's branches.
- During a Codex outage the ruleset bypass works on the bottom layer only: GitHub merges a stack's bottom pull
  request by bypass, never the whole stack, and `gh stack merge` cannot bypass at all. The admin bypass-merges each
  bottom layer in turn from its pull request page (or `gh pr merge --admin --rebase`) and states the reason in it.

## Review gate

A pull request merges when all required checks pass: `fast`, `clippy`, `test`, `python`, `deny`, `pr-meta`,
`privacy` and `codex-review`. Codex reviews automatically when a pull request is opened or marked ready for review;
drafts may be skipped. After new commits, or to review a draft, comment `@codex review`. Until a review of the
head exists, `codex-review` is pending, not failed. Fix or answer every finding and resolve every conversation.
`codex-review` passes only for a completed review of the current head with no unresolved findings. Paths in
[CODEOWNERS](.github/CODEOWNERS) also need the maintainer's approval.

When Codex is down, a repository admin merges through the ruleset bypass and states the reason in the pull request.
The bypass is for outages only, never to skip a finding.

## Risk tiers

- **High, hardware:** hardware register writes (`hal/src/dho`, `fpgad`), calibration, update packages, signing,
  kexec, boot, kernel, rootfs. Label `risk/hardware`, code owner review, `tools/plan-matrix.sh` before and after in
  the pull request, and a run on a real unit before merge (`needs-unit` until then).
- **High, pipeline:** the workflows, the gate, the review rules and their settings (`.github/`, `xtask/`,
  `tools/ci/`, `tools/privacy/`, `.githooks/`). Code owner review; no `risk/hardware` label and no unit run.
- **Medium:** behaviour visible on the scope (UI, SCPI, DSP results). Golden pictures and tests; a unit run when the
  change touches acquisition timing.
- **Low:** host-only tooling, docs, tests.

## Unit testing

Say in the pull request whether it ran on a unit, which model, which build and what was exercised. Host-only is a
valid answer for medium and low risk. A change that still needs a unit run carries the `needs-unit` label until
someone with a unit reports it. Never claim a unit test that did not happen.

## Flaky tests

A flaky test is fixed, not retried. It may be ignored only with `#[ignore = "flaky: #NNN reason"]` and an open issue
labelled `flaky`; the gate rejects a bare `#[ignore]` and a `flaky:` reason without an issue number.

## Style

- Text and code comments are in English. Changes to the hardware knowledge go into `docs/`, in the same pull request.
- Files: UTF-8, LF, a final newline, no trailing spaces, the `SPDX-License-Identifier: GPL-2.0-or-later` line in every
  source file. [.editorconfig](.editorconfig) sets this up in most editors.
- Rust: `cargo fmt` with the tree's [rustfmt.toml](rustfmt.toml)
  ([ADR 0016](docs/adr/process/0016-rust-code-formatted-by-rustfmt.md)); the tree is fully formatted. Lints are set
  per crate and their findings may only go down
  ([ADR 0032](docs/adr/process/0032-lints-and-limits-ratchet-down-only.md), [docs/development.md](docs/development.md)).
- Commits follow [AGENTS.md](AGENTS.md#commits): subject in the imperative mood, under 72 characters; the body says
  why; one topic each; formatting and moves in commits of their own; signed off. A change people would notice on the
  scope gets a `CHANGELOG.md` entry ([ADR 0019](docs/adr/process/0019-one-style-for-text-files-and-commits.md)).
- An upstream OOS change enters by cherry-pick with its author kept and the trailer
  `Cherry-picked-from: upstream <hash>` (ADR 0030).

A change that decides something others will build on (a structure, a rule of the screen, a way to treat the hardware)
comes with a decision record: [docs/adr](docs/adr/README.md), numbered from 0030 on.

## Working with a coding agent

Most code here is written with one. [AGENTS.md](AGENTS.md) holds the instructions for all of them (`CLAUDE.md` points to
it); point your tool to it if it does not read it by itself. The agent's report counts like yours: it says
what was tested on a unit, what only on the host, and what was not tested. An agent does not write to a scope unless
its owner asked for it ([ADR 0018](docs/adr/process/0018-one-agents-md-for-people-and-agents.md)).

- Agents work only on issues labelled `agent-ready`, one pull request per issue, within the issue's acceptance
  criteria and its out-of-scope list. Small pull requests; refactors and other multi-step work as a stack.
- Agents push and open pull requests only as the GitHub App `yottabyte-oos-agent[bot]`, never with a maintainer's
  credentials. Its pull requests get the `agent` label and pass the same checks; the app has no ruleset bypass.
- Agents do not touch high-risk paths unless the issue carries `risk/hardware` and says so.
- Agents run `cargo xtask check --fast` before every commit and push, push early, and report what ran locally and
  in CI.
- Agents answer Codex findings by fixing them or by a reply with the reason; they never resolve a thread without
  either.

## Licence

By contributing you agree that your work is published under the licence of the project,
[GPL-2.0-or-later](LICENSE).
