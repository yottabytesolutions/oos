# ADR 0025: Remote services are unauthenticated bench-LAN services, each with a switch, limited by `net.allow`

- Status: accepted
- Date: 2026-11-17
- Category: system

## Context

The web remote, SCPI and VXI-11 can change everything on the scope, and the web remote also takes updates. SCPI and
VXI-11 have no authentication in their protocols, and VISA programs and scripts expect none. A scope stands on a bench
LAN or on a cable to one computer; some stand in a lab network with many hosts. Telnet is a shell without password for
bring-up; SSH ([ssh.md](../../ssh.md)) is the authenticated shell.

## Decision

- The web remote, SCPI and VXI-11 accept every connection without authentication. They are meant for a bench LAN, and
  the documents say so where they name the port.
- Each network service has a switch in System > Network > Remote services (`remote.*` in `/oos/system.conf`). A switch
  turned off closes the port at once, with its connections, and the service is no longer announced. Web, SCPI, VXI-11
  and mDNS are on by default. Telnet is to be off by default; this default is pending implementation. SSH is the
  authenticated shell and is off unless the image or the user switches it on.
- `net.allow` lists the address prefixes that may reach the scope. The firewall (`oos-firewall`) drops everything
  else, and oos-ui checks the list a second time for what can change the scope: `POST /update`, `/mcp`, SCPI and
  VXI-11. Loopback is always allowed. Without `net.allow` everyone on the network is allowed.
- An update over the web must carry a trusted signature (ADR 0012); the open port does not make an unsigned image
  installable.
- A new network service comes with its switch, its line in [network.md](../../network.md) 4, and the `net.allow` check
  when it can change the scope.

## Alternatives

- **A password or token for SCPI and VXI-11:** the protocols have no place for it, and VISA programs would not send it.
- **Everything off by default:** a new scope would not be reachable for the scripts it is bought for; the web remote is
  also the update path.
- **Only the firewall:** one mistake in a rule set and everything is open; the second check in oos-ui keeps the
  services that change the scope closed.

## Consequences

- The current implementation still enables telnet by default: `SysConf::default()` sets `remote_telnet` to `true`,
  and `rcS` starts `telnetd -l /bin/sh` unless `remote.telnet` is explicitly `off`, `false` or `0`.
  Both paths must change before the target default can be documented as implemented.
- On an open network anyone can control the scope. Telnet exposes an unauthenticated root shell;
  switch it off and use SSH for authenticated shell access. `net.allow` limits network access
  ([network.md](../../network.md) 4 and 6).
- The live picture, `/cmd` and the downloads stay open to the firewall's prefixes; only the changing paths have the
  second check.
- How it is known (ADR 0015): the address lists are tested on the host (`netacl` tests), the rule set loads into a
  Linux kernel in an arm64 container; the firewall on the scope is not yet verified on hardware.
- Use on shared networks may require an authenticated transport, such as SCPI through SSH, and a revised decision.

## References

`ui/src/netacl.rs`, `ui/src/ui/remote.rs`, `ui/src/http.rs`, `ui/src/main.rs`, `rootfs/initramfs` (`oos-firewall`),
[network.md](../../network.md) 4, 6 and 7, [ssh.md](../../ssh.md), ADR 0012, ADR 0022, ADR 0024.
