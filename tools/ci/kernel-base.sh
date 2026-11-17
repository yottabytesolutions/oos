#!/bin/sh
# SPDX-License-Identifier: GPL-2.0-or-later
# The kernel base archive (kernel/README.md) is too large for Git. It lives as the single asset of the pre-release
# `kernel-base-<version>` of this repository, where image.yml downloads it. kernel/base.sha256 names and checks it.
#
#   tools/ci/kernel-base.sh name                         the release tag and the file name
#   tools/ci/kernel-base.sh download [OWNER/NAME]        into kernel/src, checked (image.yml)
#   tools/ci/kernel-base.sh publish [--apply] [OWNER/NAME] [--from OWNER/NAME TAG]
#
# publish takes the same file from a release of another repository (default: upstream OOS v0.1.13), checks it
# against kernel/base.sha256 and creates the pre-release with it. Without --apply it only downloads and checks.
# Idempotent: an existing release with a matching asset is left alone. Needs `gh` with contents: write on OWNER/NAME.
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
sums="$root/kernel/base.sha256"

line=$(grep -E '^[0-9a-f]{64}  src/linux-[0-9.]+-base\.tar\.xz$' "$sums") ||
    { echo "$sums: no archive line" >&2; exit 2; }
sha=${line%% *}
file=${line##*src/}
version=${file#linux-}
version=${version%-base.tar.xz}
tag="kernel-base-$version"

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | cut -d' ' -f1
}

check() {
    got=$(sha256 "$1")
    [ "$got" = "$sha" ] || { echo "$1: sha256 $got, kernel/base.sha256 says $sha" >&2; exit 1; }
}

command=${1:-}
[ $# -gt 0 ] && shift
case $command in
name)
    echo "$tag $file"
    ;;
download)
    repo=${1:-${GITHUB_REPOSITORY:-yottabytesolutions/oos}}
    mkdir -p "$root/kernel/src"
    gh release download "$tag" --repo "$repo" --pattern "$file" --dir "$root/kernel/src" --clobber
    check "$root/kernel/src/$file"
    echo "$file: checked against kernel/base.sha256"
    ;;
publish)
    apply=false
    if [ "${1:-}" = --apply ]; then apply=true; shift; fi
    repo=${1:-yottabytesolutions/oos}
    [ $# -gt 0 ] && shift
    from=dekajoule/oos
    from_tag=v0.1.13
    if [ "${1:-}" = --from ]; then from=$2; from_tag=$3; fi
    if gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].digest' 2>/dev/null |
        grep -qx "sha256:$sha"; then
        echo "$repo $tag already holds $file with sha256 $sha"
        exit 0
    fi
    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    gh release download "$from_tag" --repo "$from" --pattern "$file" --dir "$work"
    check "$work/$file"
    echo "$from $from_tag: $file checked against kernel/base.sha256"
    notes="The kernel source image.yml builds from: $file, sha256 $sha (kernel/base.sha256), taken from $from"
    notes="$notes $from_tag. Not an OOS release; kernel/README.md describes the tree. Made by tools/ci/kernel-base.sh."
    set -- gh release create "$tag" "$work/$file" --repo "$repo" --prerelease --latest=false \
        --title "Kernel base $version" --notes "$notes"
    printf '+ %s\n' "$*"
    if [ "$apply" = true ]; then "$@"; fi
    ;;
*)
    sed -n '6,8p' "$0" >&2
    exit 2
    ;;
esac
