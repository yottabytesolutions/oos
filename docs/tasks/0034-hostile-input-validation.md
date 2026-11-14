---
id: 34
title: "Validate hostile inputs and unsafe paths"
kind: feature
status: planned
release: 0.3.0
owner: none
depends: []
---

# Validate hostile inputs and unsafe paths

## Goal

Find parser and memory errors that line coverage misses.

## Plan

Reserved paths: none. Reserve paths when claimed.

Add bounded fuzz targets for SCPI, HTTP, setup, update, package and archive parsing with synthetic corpora.
Run supported host unsafe paths under Miri and GPU C tests under sanitizers. Use a named required or scheduled tier
and retain reproducible failures. Audit item: 24.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
