---
id: 24
title: "Scan changes and commit messages for private data"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: []
---

# Scan changes and commit messages for private data

## Goal

Catch private data before commits reach publication.

## Plan

Reserved paths: none. Reserve paths when claimed.

Before parallel migration, scan staged changes locally and incoming diffs and messages in CI.
Commit generic patterns. Keep private strings in an untracked local file. Use a reviewed allowlist for synthetic
fixtures. Remove default device addresses. Audit item: 8.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
