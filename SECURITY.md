# Security

## Supported versions

| Version | Supported |
|---|---|
| The latest release | yes |
| `main` | yes |
| Older releases | no: update to the latest release first |

## Reporting a vulnerability

Report it privately through
[GitHub private vulnerability reporting](https://github.com/yottabytesolutions/oos/security/advisories/new)
(Security > Report a vulnerability). Never open a public issue, discussion or pull request for a vulnerability.

Say what is affected, which release or commit, and how to reproduce it. A proof of concept helps. Leave out serial
numbers, MAC addresses, addresses of your network and calibration data.

## Scope

Please report:

- update packages: a package that installs with a damaged or missing signature, a header or image hash that is not
  checked, or a way around the downgrade and read-back checks of `oos-update` and the update page;
- boot and kexec: a way to start code that was not installed, or a change that keeps the stock system from
  starting;
- network services: a crash of `oos-ui` from network input, a way past the web remote's cross-site checks, or a
  service that listens although it is switched off;
- FPGA access: a register write from a path that should not reach the hardware;
- a crash of `oos-ui` from a file;
- anything that writes the FPGA flash, `/rigol/data`, or the `uboot`, `trust` and `parameter` partitions.

Behaviour that can damage hardware or lose calibration counts as a security issue, also without an attacker: a
register sequence that overdrives the analog front end, a damaged file that destroys calibration, a change that can
leave a unit unable to boot.

## Threat model

**OOS has no authentication apart from SSH, by design for now.** SSH asks for the scope's own password or a trusted key; telnet gives a root shell without a password (switch it off
once SSH is set up). SCPI, VXI-11, HiSLIP (on a computer), the web remote, the LAN announcement,
MCP and the update page are open to everyone who can reach the scope, over Ethernet and over WLAN. The community signing key
is public, so anyone on that network can also install an update. Keep the scope on a trusted bench network and never
expose it to the internet. None of this is a vulnerability to report: it is listed in the
[disclaimer](wiki/Disclaimer.md).

Signed updates protect against damaged packages, not against an attacker: see [docs/update.md](docs/update.md).

Web pages on other sites are kept out: the web remote refuses a request whose `Host` is a foreign name, whose `Origin` is
not the scope, or that the browser marks as cross-site. That needs a browser that sends `Sec-Fetch-Site` (all current
ones). It does nothing against someone on your network, which is the case above.

USB sticks count as foreign data: they are mounted `nodev,nosuid,noexec`, and a save refuses links and anything that is not
a regular file.

Out of scope: the open services and the public community key above, the stock firmware, and attacks that need an
opened unit.

## What to expect

- An acknowledgement within 7 days.
- An assessment and a plan within 30 days. OOS is maintained in spare time; these are targets, not promises.
- The fix is prepared in a private advisory fork and released before the advisory is published. You are credited
  unless you ask not to be.
