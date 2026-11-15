---
id: 24
title: "Scan changes and commit messages for private data"
kind: feature
status: active
release: 0.2.0
owner: codex-c
depends: []
---

# Scan changes and commit messages for private data

## Goal

Catch private data before commits reach publication.

## Plan

Reserved paths: tools/privacy/, .github/workflows/, .gitignore, docs/development.md (privacy section). No xtask/ until G1 merges.

Before parallel migration, scan staged changes locally and incoming diffs and messages in CI.
Commit generic patterns. Keep private strings in an untracked local file. Use a reviewed allowlist for synthetic
fixtures. Remove default device addresses. Audit item: 8.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0024-privacy-checks by the coordinator, base overhaul.
