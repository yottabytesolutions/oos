---
id: 40
title: "Implement task commands and link checks"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: [1, 41]
---

# Implement task commands and link checks

## Goal

Implement the deferred tooling in task 0001 after the xtask rewrite merges.

## Plan

Reserved paths: none. Reserve paths when claimed.

Follow [task 0001, cargo xtask tasks](0001-docs-and-tasks.md#cargo-xtask-tasks).
Implement board filters, schema and dependency checks, relative Markdown file and anchor checks, retained-task
release notes, atomic new/claim commands and overlapping reservation checks. Use plain Rust and table-driven tests.
Integrate validation into fast and full gates. Keep front matter at exactly seven keys; read reservations from Plan.

## Status

- 2026-11-14: explicitly deferred by the owner until the xtask rewrite branch is merged. No tooling is added in task 0001.
