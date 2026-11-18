# Serve raw SCPI, VXI-11 and HiSLIP with live discovery

- Number: 0019
- Category: system
- State: target: M15b scope HiSLIP enablement with remote.hislip and discovery lifecycle
- Date: 2026-11-18

## Context

Direct socket scripts use raw SCPI, while VISA clients use VXI-11 or HiSLIP. These carry the same programs with
different framing, so separate execution engines would add no compatibility. Raw SCPI and VXI-11 ran on a DHO924S over
Ethernet for 0.1.6; that report did not establish a VISA session or HiSLIP scope support.

## Decision

Serve raw SCPI on TCP 5555, VXI-11 portmapper on TCP/UDP 111 with core TCP 9009, and HiSLIP on TCP 4880. Additional
instrument protocols require a record. Announce listening services through mDNS/DNS-SD (_scpi-raw._tcp, _vxi-11._tcp,
_hislip._tcp, _lxi._tcp) and provide GET /lxi/identification. Announce a service only while it listens. --serve binds
loopback and announces nothing. Transport adapters frame messages; IO and ACLs belong behind the IO port (ADR 0023).
Only the interpreter executes SCPI (ADR 0024). Scope HiSLIP enablement must add remote.hislip, off until unit
verification; that key does not exist yet.

Check: netio transport tests and mDNS lifecycle tests; review checks announced services against listeners.

## Alternatives

Raw SCPI only: rejected because VISA clients need instrument protocol framing and discovery. Advertise configured rather
than listening services: rejected because clients would discover unavailable endpoints. Separate execution engines per
protocol: rejected because all three carry the same SCPI programs.

## Consequences

Raw SCPI and VXI-11 are in force. HiSLIP has a netio transport; scope enablement and verification remain M15b. VISA
sessions are not yet verified on hardware.
