---
id: 53
title: "Report the build in *IDN?"
kind: fix
status: planned
release: 0.2.0
owner: none
depends: []
---

# Report the build in *IDN?

## Goal

`*IDN?` names the running build, so remote tools and unit rounds can tell builds apart.

## Plan

Reserved paths: none. Reserve paths when claimed.

Today it reports only `oos-ui 0.1.12`; the build id is in /etc/oos-build. Add it to the firmware field without breaking the IDN field count; test the reply format.

## Status

- 2026-11-15: seeded by the coordinator.
