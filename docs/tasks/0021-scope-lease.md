---
id: 21
title: "Lease scope access and separate capabilities"
kind: feature
status: planned
release: 0.2.0
owner: none
depends: []
---

# Lease scope access and separate capabilities

## Goal

Require an exclusive scope lease before host tools access the shared unit.

## Plan

Reserved paths: none. Reserve paths when claimed.

Before parallel migration work, add an atomic directory on the device with holder, task, build and heartbeat.
Host tools must refuse access without the lease. Separate observe, test, install and reboot capabilities.
Install and reboot need the owner's go for the task. Quarantine expired leases; never transfer automatically.
Make tools/device-deploy.sh owner-only. Remove default device addresses from tools.
Audit items: 3, 4, 8. No hardware access is authorized by this task file.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
