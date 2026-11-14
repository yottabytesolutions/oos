---
id: 20
title: "Enforce the shared agent rules"
kind: chore
status: planned
release: 0.2.0
owner: none
depends: [1]
---

# Enforce the shared agent rules

## Goal

Keep universal rules in AGENTS.md and each agent entry file a thin pointer.

## Plan

Reserved paths: none. Reserve paths when claimed.

The documentation is prepared in task 0001. Add a gate check that rejects independent rules in the shims.
Allow only the Claude session trailer in CLAUDE.md. Document explicit AGENTS.md loading for other clients.
Audit item: 1.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
