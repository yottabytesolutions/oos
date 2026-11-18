# Unverified hardware functions are off by default

- Number: 0012
- Category: hardware
- State: target: system.conf-only switches and off-until-verified defaults; hardware-key inventory follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

Host tests cannot prove that a new FPGA sequence works on a unit. Verified on the unit: DHO924S, 2026-11-15, autoset
read stale levels from the FPGA measurement unit and chose the wrong scale. Shipping an unverified path as the default
exposes every owner to a failure that a host-only test cannot detect.

## Decision

Every hardware statement in docs and work reports says one of: verified on the unit, assumed, or not yet verified on
hardware. Verified means observed working on a running scope and recorded in docs with model and date, always.

An unverified hardware function merges switched off. Its only switch is a key in `/oos/system.conf`. The default is what
a generic image does with an empty `/oos`. Each temporary key has an open removal issue and a row in
docs/system-conf-keys.md, the sole inventory. After verification the function becomes default and the off path is
deleted. After failure the function is deleted.

Check: review only today; the target text check matches parsed keys to inventory rows. The follow-up reconciles existing
image defaults, environment switches and missing removal issues before claiming full enforcement.

## Alternatives

Enable a host-tested function immediately: rejected after the stale-level autoset failure on a real unit. Keep permanent
experiment switches: rejected because each adds an unverified second path without a removal deadline.

## Consequences

Each temporary switch adds a second path to test until verification permits deleting it. Existing enabled
experiments still need recorded unit evidence.
