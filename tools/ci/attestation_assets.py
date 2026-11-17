# SPDX-License-Identifier: GPL-2.0-or-later
"""Attach an action's Sigstore bundle and exposed in-toto statement beside each release asset."""
import argparse
import base64
import json
from pathlib import Path


def attach(bundle: Path, assets: list[Path]) -> None:
    data = bundle.read_bytes()
    payload = json.loads(data).get("dsseEnvelope", {}).get("payload")
    statement = None
    if payload is not None:
        statement = json.dumps(json.loads(base64.b64decode(payload, validate=True)), separators=(",", ":")) + "\n"
    for asset in assets:
        asset.with_name(asset.name + ".sigstore.json").write_bytes(data)
        if statement is not None:
            asset.with_name(asset.name + ".intoto.jsonl").write_text(statement, encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("assets", type=Path, nargs="+")
    args = parser.parse_args()
    attach(args.bundle, args.assets)
