# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
# deps.sh - sourced by the rootfs build scripts: their sources from rootfs/deps.lock (docs/fork/deps.md).
#   dep_ver NAME                       the pinned version
#   dep_fetch NAME                     download into $DEPS_SOURCES once (the url, then its mirrors), check the
#                                      SHA-256, print the path
#   dep_stale ARTIFACT SCRIPT NAME...  true when ARTIFACT is missing or was built from other manifest lines or another
#                                      SCRIPT (its build options): boot/build-dev-image.sh rebuilds it then
#   dep_stamp ARTIFACT SCRIPT NAME...  record what ARTIFACT was built from (ARTIFACT.stamp), at the end of a build
DEPS_LOCK="${DEPS_LOCK:-$here/deps.lock}"
deps_root=$(CDPATH='' cd -- "$(dirname -- "$DEPS_LOCK")/.." && pwd)
DEPS_SOURCES="${DEPS_SOURCES:-$deps_root/rootfs/out/sources}"
. "$deps_root/tools/lib/portable.sh"
dep_line() {
	awk -v n="$1" '!/^#/ && $1 == n { print; f = 1 } END { exit !f }' "$DEPS_LOCK" || { echo "deps.lock: no $1" >&2; return 1; }
}
dep_ver() { dep_line "$1" | awk '{ print $3 }'; }
dep_sha256() { sha256 "$1"; }
# dep_urls URL: URL, then the mirrors of its host
dep_urls() {
	echo "$1"
	path=${1#https://*/}
	case "$1" in
	https://mirrors.edge.kernel.org/*) echo "https://cdn.kernel.org/$path"; echo "https://www.kernel.org/$path" ;;
	https://cdn.openbsd.org/*) echo "https://ftp.openbsd.org/$path" ;;
	https://matt.ucc.asn.au/dropbear/releases/*) echo "https://dropbear.nl/mirror/releases/${1##*/}" ;;
	https://www.netfilter.org/projects/*/files/*)
		project=${1#https://www.netfilter.org/projects/}
		echo "https://ftp.netfilter.org/pub/${project%%/*}/${1##*/}" ;;
	esac
}
# a dead host costs at most 4 connects of 20 s; curl backs off between them
dep_download() {
	for u in $(dep_urls "$2"); do
		curl -fsSL --connect-timeout 20 --max-time 900 --retry 3 --retry-all-errors -o "$1.part" "$u" &&
			mv "$1.part" "$1" && return 0
		echo "download failed: $u" >&2
		rm -f "$1.part"
	done
	return 1
}
dep_fetch() {
	l=$(dep_line "$1") || return 1
	sum=$(echo "$l" | awk '{ print $4 }'); url=$(echo "$l" | awk '{ print $5 }')
	t="$DEPS_SOURCES/$(basename "$url")"
	mkdir -p "$DEPS_SOURCES"
	[ -f "$t" ] || dep_download "$t" "$url" || return 1
	[ "$(dep_sha256 "$t")" = "$sum" ] || { echo "wrong checksum: $t (deps.lock $1)" >&2; rm -f "$t"; return 1; }
	echo "$t"
}
dep_stamp_text() {
	s=$2; shift 2
	script_sum=$(dep_sha256 "$s") || return 1
	helper_sum=$(dep_sha256 "$deps_root/rootfs/deps.sh") || return 1
	echo "script $script_sum"
	echo "helper $helper_sum"
	deps_environment=$("$deps_root/tools/build-env.sh" identity "rootfs-$(basename "$s" .sh | sed 's/^build-//')") || return 1
	echo "environment $deps_environment"
	for n in "$@"; do
		line=$(dep_line "$n") || return 1
		printf '%s\n' "$line" | awk '{ print $1, $3, $4 }'
	done
}
dep_stale() {
	a=$1; shift
	[ -e "$a" ] && [ -f "$a.stamp" ] || return 0
	expected=$(dep_stamp_text "$a" "$@") || return 0
	[ "$(cat "$a.stamp")" != "$expected" ]
}
dep_stamp() {
	a=$1; shift
	dep_stamp_text "$a" "$@" > "$a.stamp"
}
