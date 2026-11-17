# ADR 0024: The scope serves raw SCPI, VXI-11 and HiSLIP with LAN discovery

- Status: accepted
- Date: 2026-11-17
- Category: system

## Context

Raw SCPI supports direct socket scripts. VXI-11 and HiSLIP support VISA clients. All three carry the same SCPI
programs; their duplicated connection handling belongs in a shared remote IO layer rather than in each transport.

## Decision

- The scope serves raw SCPI on TCP 5555, VXI-11 (`TCPIP::<host>::INSTR`) through its portmapper on TCP/UDP 111
  and core channel on TCP 9009, and HiSLIP (`TCPIP::<host>::hislip0::INSTR`) on TCP 4880.
- Discovery uses mDNS / DNS-SD (`_vxi-11._tcp`, `_scpi-raw._tcp`, `_hislip._tcp`, `_lxi._tcp`) and the web remote's
  `GET /lxi/identification`. Announce only services that are listening.
- On a computer, `oos-ui --serve` is the browser view (ADR 0036). It binds its services to loopback, prints its
  ports and announces nothing. There is no separate desktop build in this tree.
- All three transports share the interpreter (ADR 0022) and move onto the shared remote IO layer on the async IO
  thread (ADR 0037, architecture plan M15a). Each keeps only its own framing. The duplicated accept, read, ACL,
  size-cap and close code moves into that layer.

## Consequences

VISA clients can use VXI-11 or HiSLIP; direct scripts use raw SCPI. HiSLIP and VXI-11 are streamlined by sharing
connection handling while retaining their framing. The shared IO migration and HiSLIP service on the scope are
planned, not implemented by this record. Raw SCPI and VXI-11 servers ran on a DHO924S over Ethernet
(CHANGELOG 0.1.6); a VISA session on the unit is not recorded. HiSLIP currently runs on a computer only and is not
yet verified on hardware. mDNS was tested on the host only, not yet on hardware.

## References

[network.md](../../network.md), [scpi.md](../../scpi.md), [architecture plan](../../architecture/migration.md#m15-remote-adapters-on-commands-and-queries)
(M15a), ADR 0022, ADR 0025, ADR 0036, ADR 0037.
