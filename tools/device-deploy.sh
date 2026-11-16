#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# OWNER-ONLY: requires the owner's go for this task and an install lease.
# Agents must use signed packages through oos-update or the web update page.
# device-deploy.sh - runs the oos-ui of this tree from /tmp on a running OOS.
# Programs stay in RAM; the lease check writes coordination metadata under /oos.
# The next power cycle is back to the installed image. No reboot is needed, and none would work: from OOS to
# OOS takes a power cycle.
#
#   tools/device-deploy.sh --host scope.example
#     upload oos-ui, oos-fpgad and plan-matrix.sh, switch to the new oos-ui
#   tools/device-deploy.sh --host scope.example --restore   back to the installed oos-ui
#
# Build first: python3 tools/output.py build --release --target aarch64-unknown-linux-musl -p oos-ui -p oos-fpgad.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
root=$here
. "$root/tools/output.sh"
host=${OOS_SCOPE:-}
restore=0
while [ "$#" -gt 0 ]; do
	case "$1" in
		--host) [ "$#" -ge 2 ] || { echo "--host needs a value" >&2; exit 1; }; host=$2; shift 2 ;;
		--restore) restore=1; shift ;;
		*) echo "usage: device-deploy.sh [--host HOST] [--restore]; or set OOS_SCOPE" >&2; exit 1 ;;
	esac
done
[ -n "$host" ] || { echo "specify --host or set OOS_SCOPE; no default scope" >&2; exit 1; }
"$here/tools/scope-lease" status --host "$host" --cap install
port=${OOS_SSH_PORT:-22}
ui=${OOS_DEPLOY_UI:-$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/release/oos-ui}
fpgad=${OOS_DEPLOY_FPGAD:-$CARGO_TARGET_DIR/aarch64-unknown-linux-musl/release/oos-fpgad}

# SSH never accepts a changed/unknown identity silently. Provision and verify
# the scope host fingerprint first (docs/ssh.md). The optional identity stays local.
remote() {
	if [ -n "${OOS_SSH_IDENTITY:-}" ]; then
		ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -p "$port" -i "$OOS_SSH_IDENTITY" "root@$host" "$1"
	else
		ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -p "$port" "root@$host" "$1"
	fi
}

send() {
	echo "sending $(basename "$1") -> /tmp/$2"
	remote "umask 077; cat > /tmp/$2.part && mv /tmp/$2.part /tmp/$2" < "$1"
	if command -v shasum >/dev/null 2>&1; then want=$(shasum -a 256 "$1" | cut -d' ' -f1)
	else want=$(sha256sum "$1" | cut -d' ' -f1); fi
	[ -n "$want" ] || { echo "error: no SHA256 tool" >&2; exit 1; }
	remote "sha256sum /tmp/$2" | grep -q "^$want " || { echo "error: /tmp/$2 differs after the upload" >&2; exit 1; }
	remote "chmod +x /tmp/$2" >/dev/null
}

loop() {
	# as rcS starts it, with another program and another stop file; the hardware is up, so no start-up screen
	remote "(setsid sh -c 'while [ ! -e $2 ]; do $1 --hw --afe >> /tmp/oos-ui.log 2>&1; sleep 2; done' </dev/null >/dev/null 2>&1 &)" 1 >/dev/null
}

if [ "$restore" -eq 1 ]; then
	remote "touch /tmp/oos-ui.off2; kill \$(pidof oos-ui.new) 2>/dev/null || true" 3 >/dev/null
	remote "rm -f /tmp/oos-ui.off" 1 >/dev/null
	loop /bin/oos-ui /tmp/oos-ui.off
	echo "installed oos-ui running again"
	exit 0
fi

python3 "$root/tools/output.py" verify "$ui" "$fpgad"
[ -x "$ui" ] && [ -x "$fpgad" ] || { echo "error: build first (see the top of this script)" >&2; exit 1; }
remote "cat /proc/version; ls -l /bin/oos-ui; pidof oos-ui oos-ui.new; df -k /tmp | tail -1" 2
# a build from an earlier deploy still runs from /tmp: "text file busy" for the upload otherwise
remote "touch /tmp/oos-ui.off2; kill \$(pidof oos-ui.new) 2>/dev/null || true" 3 >/dev/null
send "$ui" oos-ui.new
send "$fpgad" oos-fpgad.new
send "$here/tools/plan-matrix.sh" plan-matrix.sh
remote "touch /tmp/oos-ui.off; kill \$(pidof oos-ui) 2>/dev/null || true" 3 >/dev/null
remote "rm -f /tmp/oos-ui.off2" 1 >/dev/null
loop /tmp/oos-ui.new /tmp/oos-ui.off2
sleep 6
remote "pidof oos-ui.new; tail -3 /tmp/oos-ui.log" 2
