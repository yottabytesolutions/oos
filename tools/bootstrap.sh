#!/bin/sh
# Explicit host setup. Install Python >=3.12, rustup and Docker before running this.
set -eu
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
for flag in "$@"; do
    case "$flag" in --gpu|--sim|--image) ;; *) echo 'usage: tools/bootstrap.sh [--gpu] [--sim] [--image]' >&2; exit 2 ;; esac
done
python3 -I - <<'PY'
import json, pathlib, sys, tomllib
root = pathlib.Path.cwd()
tools = json.loads((root / 'tools/build-tools.json').read_text())
if sys.version_info[:2] < tuple(map(int, tools['python'].split('.'))):
    sys.exit('install Python >= ' + tools['python'] + ' as python3 first')
PY
. "$root/tools/output.sh"
rust_pin=$(python3 -I -c 'import tomllib; print(tomllib.load(open("rust-toolchain.toml", "rb"))["toolchain"]["channel"])')
rustup toolchain install "$rust_pin" --profile minimal --component rustfmt,clippy,llvm-tools-preview --target aarch64-unknown-linux-musl
coverage_pin=$(python3 -I -c 'import json; print(json.load(open("tools/build-tools.json"))["cargo-llvm-cov"])')
cargo install cargo-llvm-cov --version "=$coverage_pin" --locked
cargo fetch --locked
for flag in "$@"; do
    roles=$(python3 -I -c 'import json,sys; p=json.load(open("tools/build-tools.json"))["profiles"]; names=[sys.argv[1][2:]]; names = ["gpu","sim","image"] if names == ["image"] else names; print(" ".join(dict.fromkeys(r for n in names for r in p[n])))' "$flag")
    for role in $roles; do tools/build-env.sh prepare "$role"; done
done
cargo xtask doctor "$@"
