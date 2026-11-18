# SSH access and trusted keys

The SSH server, its switches and its page are described in [network.md](network.md) section 7: Dropbear started by
`/bin/oos-ssh`, off in generic images (`remote.ssh`), enabled from the front panel, listening only with `net.allow` and the firewall loaded, root with the
scope's own password or a trusted key, System > Network > Remote services > SSH settings. telnet is off by default and scheduled for removal in #139
(`remote.telnet`, off by default); enable it only for bench bring-up. `net.allow` is set on the same page, **Allowed**
(network.md section 6); it applies at once: with SSH switched on, a list set starts the server, and a list removed
stops it with its sessions.

This page is about getting trusted keys onto a scope without the password.

## Generic images

Images carry no private SSH trust. Add keys on the device through its data partition or a USB stick using the
procedures below. Existing keys, host keys and passwords on `/oos/ssh` survive updates unchanged (ADR 0015).

## Keys written to a card on a computer

With the scope's data partition mounted on a computer:

```
python3 tools/ssh-keys.py --data-dir /mounted/oos add ~/.ssh/id_ed25519.pub
python3 tools/ssh-keys.py --data-dir /mounted/oos list
python3 tools/ssh-keys.py --data-dir /mounted/oos remove SHA256:EXACT_FINGERPRINT
```

The tool edits `ssh/authorized_keys` in that folder only: it mounts, formats and connects to nothing, keeps lines it
does not know, writes atomically and refuses symbolic links. New imports are bare Ed25519 keys; `list` shows SHA-256
fingerprints, `remove` takes one of them.

The other ways in, on the running scope: Import keys from USB on the SSH settings page, or `ssh-copy-id` with the
password (network.md 7).

## Tools over SSH

`tools/device-deploy.sh` and `tools/device-test.py` use the telnet shell. Over SSH instead:
`tools/device-deploy-ssh.sh <ip>` and `OOS_SHELL=ssh tools/device-test.py ...`, with strict host key checking
(compare the fingerprint on the SSH settings page with the first `ssh root@<ip>` prompt first); `OOS_SSH_IDENTITY`
names a private key, `OOS_SSH_PORT` another port.

Offline tests of the key tool: `python3 -m unittest discover -s tools/tests -p test_ssh_keys.py -v`.
