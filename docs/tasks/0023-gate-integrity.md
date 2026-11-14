---
id: 23
title: "Protect ratchets and workspace coverage"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Protect ratchets and workspace coverage

## Goal

Reject gate weakening and ensure every workspace member enters applicable checks.

## Plan

Reserved paths: none. Reserve paths when claimed.

Coordinate with auxout before the guardrails merge. Compare ratchets, required baseline entries, lints and checks
against the base. Derive workspace members from Cargo metadata and reject missing or stale gate entries.
Keep only exceptional crate properties manual. Audit items: 7, 17.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
