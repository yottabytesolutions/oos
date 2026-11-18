# One generic image per model keeps device state in oos

- Number: 0015
- Category: system
- State: target: complete device-state manifest; device-state follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

Per-unit build selectors embedded network identity, LAN prefixes and hardware defaults into different images from the
same source. An update could replace a unit's identity with image defaults. Issue #144 requires one reproducible image
per model while preserving existing device identity and capability overrides.

## Decision

One image serves each model family. It contains no per-device MAC, LAN prefix, SSH key or unit name. The model is
detected at runtime; updates match the detected model and board target (ADR 0016).

Device state lives in `/oos`, in the last 16 MiB of backup: eth.mac, net.allow, the IPv6 secret, SSH host/trusted keys
and password, capability overrides in limits.conf, and sd.mode. First boot creates absent values with defaults; an
existing value wins. Updates never overwrite this state. Durable writes use a temporary file, fsync, rename and
directory fsync. MAC precedence is saved eth.mac, stored board MAC, then a deterministic CPU-id-derived address; never
random. A device setting selects only a variant contained in the signed package, including the sd.mode device tree; it
does not otherwise change signed content.

Check: identity precedence/idempotence and update tests; privacy image-tree scan; review of durable writes. The
follow-up covers all first-boot defaults and update preservation with one complete state manifest.

## Alternatives

Per-unit images: rejected because updates can overwrite identity and identical source produces different images. Random
MAC fallback: rejected because it changes network identity; patching signed image content per device would also defeat
the generic signed-package contract.

## Consequences

Generic images and MAC precedence are in force. New boot paths are not yet verified on hardware. GMAC probe details live
in docs/network.md.
