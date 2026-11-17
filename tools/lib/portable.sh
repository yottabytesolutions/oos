# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
# portable.sh - sourced by host scripts: the commands that differ between Linux, the BSDs and macOS, in one form.
#   sha256 [FILE]     the SHA-256 of FILE (stdin without one) as hex; false when it cannot be read
#   sha256_check      checks the "HASH  FILE" lines on stdin (the sha256sum format); false on the first mismatch or
#                     on no line at all
#   file_bytes FILE   the size of FILE in bytes
# sha256sum is on Linux, FreeBSD and macOS, shasum on macOS and most Linux, openssl on every one of them (OpenBSD and
# NetBSD have neither of the first two).
sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		set -- sha256sum "$@"
	elif command -v shasum >/dev/null 2>&1; then
		set -- shasum -a 256 "$@"
	else
		set -- openssl dgst -sha256 -r "$@"
	fi
	portable_hash=$("$@" | awk '{ print $1; exit }')
	[ -n "$portable_hash" ] || return 1
	printf '%s\n' "$portable_hash"
}

sha256_check() {
	portable_lines=0
	while read -r portable_want portable_name; do
		[ -n "$portable_want" ] || continue
		portable_name=${portable_name#\*}
		portable_got=$(sha256 "$portable_name") || { echo "$portable_name: cannot read" >&2; return 1; }
		[ "$portable_got" = "$portable_want" ] ||
			{ echo "$portable_name: sha256 $portable_got, expected $portable_want" >&2; return 1; }
		portable_lines=$((portable_lines + 1))
	done
	[ "$portable_lines" -gt 0 ] || { echo "sha256_check: no checksum line" >&2; return 1; }
}

file_bytes() {
	portable_bytes=$(wc -c < "$1") || return 1
	printf '%s\n' "$((portable_bytes + 0))"
}
