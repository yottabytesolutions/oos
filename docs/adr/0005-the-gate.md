# The gate is cargo xtask check

- Number: 0005
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

The hooks, local full gate and parallel CI jobs must execute the same checks with the same prerequisites. Running the
full gate in every hook delays pushes and repeats CI work; separate shell and Rust entry points can drift. One
executable inventory gives fast local feedback and identifies the checks required on the final commit.

## Decision

`cargo xtask check` is the inventory. A check outside it is not a gate. CI runs it as jobs on GitHub-hosted runners.
`.github/rulesets/main.json` is the source of required checks. Completion requires those checks on the final commit and
accepted independent review. Hooks run only `cargo xtask check --fast`. Ratchets compare against main. One independent
automated review is required; `docs/process/ci.md` names the tool. Unit checks are required release evidence in the
release notes.

Check: main ruleset; inventory/CI consistency tests in xtask; review checks release evidence.

## Alternatives

Run the full gate in hooks: rejected because it delays pushes and repeats CI work. Maintain independent inventories in
scripts and CI: rejected because checks and prerequisites can drift.

## Consequences

The detailed inventory and prerequisites live in docs/development.md. Host performance CI still needs the follow-up in
ADR 0031.
