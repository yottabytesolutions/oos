# ADR 0031: The gate is `cargo xtask check`

- Status: accepted, amended 2026-11-17 (the full gate runs in CI, the hooks run the fast tier)
- Date: 2026-11-17
- Category: process

## Context

ADR 0020 made `tools/check.sh` the one command for the host checks and rejected `cargo xtask`. This tree has run
`cargo xtask check` as its gate since the guardrails baseline (task 0041): formatting, size limits, architecture
rules, statics, `unsafe`, method exceptions, known failures, agent rules, test timing, Clippy on host and target,
tests in debug and release, performance guards, coverage, script syntax and the Python tests. `tools/check.sh` was
kept for work sent upstream. After the hard fork (ADR 0030) there is no upstream to send work to, and two entry
points describe two different gates.

`tools/check.sh` still has two layers the xtask gate does not run: `text` (SPDX line, line ends, trailing spaces,
ADR 0019) and `decoders` (the core-only build and every optional decoder alone).

## Decision

- `cargo xtask check` is the gate. It is the one command a person, an agent and CI run. Its tiers:
  - `cargo xtask check --fast`: the seconds-long steps; the pre-commit hook.
  - `cargo xtask check`: the full default gate; the pre-push hook and the done rule. A change is done when this
    passes on its final commit.
  - `cargo xtask check --image`: the default gate plus the Docker, emulation and image checks; run before unit
    rounds and releases.
  - `cargo xtask check --list` prints the steps, their tier and their prerequisites.
- The ratchet files are compared with the protected base `main` (ADR 0032).
- `tools/check.sh` is not a gate. It stays only for its `text` and `decoders` layers until the xtask gate runs them;
  then it is removed. No new layer is added to it.
- The checks that need a baseline from before the change, a quiet machine or a scope stay outside the default gate
  and are named in the pull request when they ran: golden pictures and `ui/tools/layout-check.sh` (ADR 0014),
  `tools/plan-matrix.sh` before and after, `tools/device-test.py` on a unit with its owner's consent.
- CI runs the same inventory on GitHub-hosted runners, split into parallel jobs with `cargo xtask step <id>...`
  ([pr.yml](../../../.github/workflows/pr.yml), task 0092): the step definitions stay in xtask. The perf guards
  stay local (ratio guards are load-sensitive on shared runners). Until Actions is enabled the local full gate is
  authoritative.

## Alternatives

- **Keep `tools/check.sh` as the entry:** a POSIX script cannot hold the ratchets, the move-aware size limits and
  the architecture rules without becoming a second gate in shell.
- **Make `tools/check.sh` a two-line wrapper now:** it would drop the `text` and `decoders` layers. They move into
  xtask first.
- **Remove `tools/check.sh` now:** same loss.

## Consequences

- `AGENTS.md`, `CONTRIBUTING.md` and `README.md` name `cargo xtask check`, not `tools/check.sh`.
- `xtask` is a workspace crate with host-only dependencies (ADR 0013, ADR 0021 amendments).
- A new kind of check becomes an xtask step with its inventory line, not a script to remember.
- The default gate takes tens of minutes on a laptop; `--fast` is for the edit loop.

## Amendment 2026-11-17: the full gate runs in CI

Actions is enabled. The full gate on the pre-push hook took tens of minutes per push, so branches were pushed late
and CI's parallel jobs sat idle while a laptop repeated them.

- The pre-commit and pre-push hooks run `cargo xtask check --fast`.
- The full gate is the required CI checks of a pull request (`fast`, `clippy`, `test`, `python`, `deny`, with
  `pr-meta`, `privacy` and `codex-review`) and the `main` workflow after the merge. A change is done when the
  required checks passed on its final commit and the review accepted it.
- Run `cargo xtask check` locally when an early signal helps. `--image` and the perf guards stay local, as above.
- "Until Actions is enabled the local full gate is authoritative" and "the pre-push hook" in the full tier above no
  longer apply.

## Note 2026-11-17: xtask is the home for Rust tooling

ADR 0033 decides it: new project tooling and new checks written in Rust go into `xtask`, as steps of this gate or
subcommands; small helpers stay POSIX shell or Python scripts in `tools/`. ADR 0020's rejection of `xtask` no longer
applies.

## References

`xtask/`, [docs/development.md](../../development.md) (Executable inventory, What each check enforces),
`.githooks/`, `tools/check.sh`, ADR 0014, ADR 0019, ADR 0020 (superseded), ADR 0030, ADR 0032, ADR 0033.
