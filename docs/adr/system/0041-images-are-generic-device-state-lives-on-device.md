# ADR 0041: Images are generic; device state lives on the device

- Status: accepted
- Date: 2026-11-17
- Category: system

## Context

A per-unit image selected network identity, LAN prefixes and hardware defaults at build time. This made the same
source produce different images for different owners, and an update could replace identity with image defaults.
The owner requires one reproducible image per model and unchanged existing device identity (issue #144).

The existing HAL reader exposes a stored LAN MAC from read-only board identity data (docs/models.md). Its availability
on every unit is not yet verified on hardware. The RK3399 CPU id is the deterministic fallback, independent of the
SD card. Existing unit measurements established that stable IPv6 addresses also depend on the MAC used by addrconf.

## Decision

Images contain only shared model defaults and a signed list of supported models for their board family. Detect the
model and stored MAC at runtime; persist a chosen MAC in `/oos/system.conf`, with existing keys always taking precedence.
Build the Rockchip GMAC glue as a module. rcS resolves the MAC before loading it, passing `macaddr` so stmmac
registers that address as permanent before IPv6 sees the interface.

## Consequences

- No build selector, per-unit configuration directory, embedded private SSH trust or MAC in the device tree.
  Supported DHO models share system defaults; their capability table supplies generator and LA availability. A unit
  whose stored model differs from its physical hardware must preserve its capability overrides in `/oos/limits.conf`
  before migration; these are device data, not model-wide defaults.
- `net.allow = onlink 169.254.0.0/16 fe80::/10 onlink6` works on any LAN. IPv4 connected subnets and IPv6 on-link
  prefixes follow address changes; explicit device overrides still win.
- MAC precedence: saved `eth.mac`, stored board MAC, then a locally administered unicast address derived from the
  RK3399 CPU id. First boot creates an identity file with a header, or appends only an absent key. No random fallback.
- Updates never rewrite `/oos/system.conf` or `/oos/net/ipv6.secret`. Units whose old MAC came only from an image
  default must save the current address in `/oos/system.conf` before updating (docs/network.md).
- The stock loader cannot read `/oos/system.conf` before starting Linux. Deferring the GMAC probe works on both
  cold boot and kexec without changing that loader, patching a signed image, or changing IPv6 hashing. The driver
  requires a valid `macaddr` and rcS verifies the registered address before configuring IPv6 or bringing eth0 up.
- IPv6 stable address hashing still uses the permanent MAC. Supplying the same address at probe preserves the old
  DTB hash input. The existing stable secret stays at its original path and is unchanged.
- Update matching uses the detected model and signed board target. `--any-model` replaces `--any-unit`; it is explicit,
  logged and does not bypass board, signature or image verification. Legacy unit-named installs are not model identity.
  An old updater still needs its old override for the first generic image, as documented in docs/update.md.
- Host tests cover precedence, idempotence, derivation, probe MAC validation, DHCP changes and migration.
  Kernel/image validation and a real unit round remain required before integration; the new boot paths are not
  yet verified on hardware.

## References

- Issue #144, owner decision and identity-preservation addition.
- [Network](../../network.md), [models](../../models.md), [updates](../../update.md).
