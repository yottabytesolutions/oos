# ADR 0031: The gate is cargo xtask check

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

One inventory of checks keeps local feedback and CI consistent. Running the full gate in hooks delays pushes and
repeats CI's work.

## Decision

- `cargo xtask check` is the full default gate. The pre-commit and pre-push hooks run `cargo xtask check --fast`.
  Run the full gate locally when an early signal helps.
- Required CI checks (`fast`, `clippy`, `test`, `python`, `deny`, `pr-meta`, `privacy`, `codex-review`) are the gate
  for a pull request. Completion requires them to pass on its final commit and the review to accept it.
- CI runs the xtask inventory as parallel jobs. Its `python` job runs the Python and shell commands directly,
  without building Rust or xtask. The local `script-syntax` and `python-tests` steps run those same commands;
  changes keep both entry points aligned.
- `cargo xtask check --image` adds Docker, emulation and image checks before unit rounds and releases.
  Performance guards stay local. `cargo xtask check --list` lists steps and prerequisites without running them.
- Ratchets compare against the protected base `main` (ADR 0039). Screen proof belongs in the gate (ADR 0014);
  unit checks still require the owner's authorization.
- New Rust tooling and checks go into xtask as steps or subcommands (ADR 0033). Small helpers stay in `tools/`.
  There is no `tools/check.sh` entry point.

## Consequences

Hooks provide fast feedback; CI decides whether the final commit passes. Check definitions and prerequisites are
listed in [development.md](../../development.md#executable-inventory-and-completion-evidence).
