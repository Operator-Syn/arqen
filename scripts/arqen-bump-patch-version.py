#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Increment only the root Arqen package patch version in Cargo manifests."""

from __future__ import annotations

import argparse
import re
import tomllib
from pathlib import Path


VERSION_PATTERN = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


def bump(root: Path) -> str:
    manifest_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    with manifest_path.open("rb") as stream:
        current = tomllib.load(stream)["package"]["version"]
    match = VERSION_PATTERN.fullmatch(current)
    if match is None:
        raise ValueError(f"expected a stable numeric X.Y.Z version, found {current!r}")
    major, minor, patch = (int(part) for part in match.groups())
    updated = f"{major}.{minor}.{patch + 1}"

    manifest_text = manifest_path.read_text(encoding="utf-8")
    manifest_text, count = re.subn(
        r'(?m)^(version\s*=\s*)"' + re.escape(current) + r'"$',
        rf'\g<1>"{updated}"',
        manifest_text,
        count=1,
    )
    if count != 1:
        raise ValueError("could not uniquely update the root Cargo.toml package version")

    lock_text = lock_path.read_text(encoding="utf-8")
    root_package = re.compile(
        r'(?ms)(\[\[package\]\]\s+name\s*=\s*"arqen"\s+version\s*=\s*)"'
        + re.escape(current)
        + r'"'
    )
    lock_text, count = root_package.subn(rf'\g<1>"{updated}"', lock_text, count=1)
    if count != 1:
        raise ValueError("could not uniquely update the root arqen package in Cargo.lock")

    manifest_path.write_text(manifest_text, encoding="utf-8")
    lock_path.write_text(lock_text, encoding="utf-8")
    return updated


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    try:
        print(bump(args.root.resolve()))
    except (OSError, KeyError, tomllib.TOMLDecodeError, ValueError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
