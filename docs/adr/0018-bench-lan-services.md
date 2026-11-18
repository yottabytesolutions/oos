# Network services are for a bench LAN; SSH is the shell

- Number: 0018
- Category: system
- State: target; implemented parts and remaining checks are listed below
- Date: 2026-11-18

## Context

SCPI and VXI-11 can change the scope but have no authentication field, and VISA programs expect none. The web remote
also installs updates; bench networks may include many other hosts. Remote shells need authentication even on a
bench LAN.

## Decision

Web remote, raw SCPI, VXI-11 and HiSLIP are unauthenticated bench-LAN services, enabled by default once their scope
implementation is verified; HiSLIP scope enablement remains a target (ADR 0019). Documentation that names their ports
states this. SSH (Dropbear, password or trusted key) is the only remote shell, off on a fresh unit and enabled
from the panel. Telnet is removed from the image, boot, settings and menus. Each service has a remote.* switch
under System > Network > Remote services. Off closes listeners and connections within one second and withdraws discovery.

net.allow defaults to directly connected subnets and link-local prefixes. The firewall drops other traffic; loopback is
always allowed. oos-ui checks the current ACL again on changing paths: POST /update, /mcp, /cmd, SCPI, VXI-11 and
HiSLIP. Every new service has its switch, ACL check and network.md row.

Check: netacl and transport tests; review. Service-table coverage, one-second closure and /cmd ACL remain
target follow-ups in [migration](../architecture/migration.md#adr-baseline-follow-ups).

## Alternatives

A password or token for SCPI/VXI-11: rejected because the protocols have no field and VISA clients send none. Everything
off by default: rejected because scripts and the web update path need reachability; SSH remains opt-in. Only the
firewall: rejected because one ruleset error would expose all changing paths without the second ACL.

## Consequences

Old `remote.telnet` settings load without error as inert unknown lines. Shell tools use key-only SSH with strict
host key checking. Telnet removal and firewall enforcement on the scope are not yet verified on hardware. /cmd
currently filters commands with cmd::remote_ok but lacks the second ACL check.
