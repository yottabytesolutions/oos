# ADR 0018: One AGENTS.md holds the instructions for people and agents

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

Coding agents load different entry files. Repeating repository rules in each file makes them drift apart.

## Decision

- [AGENTS.md](../../../AGENTS.md) is the shared instruction file for people and agents. Keep it short by pointing
  to the architecture, contribution rules and decision records, without a hard line limit.
- Client entry files, including [CLAUDE.md](../../../CLAUDE.md), only load AGENTS.md. Personal permissions, memory,
  local paths and unit addresses stay out of the tree.
- Where AGENTS.md and an ADR differ, the ADR holds; correct AGENTS.md in the same pull request. New decisions get
  an ADR; changes to an existing decision rewrite its record in place.
- Agents do not write to a scope unless its owner authorized that work. They report what was tested on a unit,
  what only on the host and what was not tested (ADR 0015).
- Agents run `cargo xtask check --fast` and focused tests; the required CI checks and accepted review determine
  completion (ADR 0031). After the hard fork (ADR 0030), upstream work requires the owner's request.

## Consequences

Rules have one home. Entry files and the commands they name must stay accurate as the rules change.
