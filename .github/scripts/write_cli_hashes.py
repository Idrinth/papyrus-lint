#!/usr/bin/env python3
"""Bake PapyrusLinterCLI and PapyrusLinter SHA-256 digests into the editor
plugins.

The hashing/rendering logic lives in ci_lib/cli_release_hashes.py; this is
just the CLI entrypoint.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.cli_release_hashes import PY_PATH, TS_PATH, collect_hashes, write_hashes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "asset_dir",
        type=Path,
        help="directory containing the PapyrusLinterCLI assets and optional PapyrusLinter GUI binaries",
    )
    parser.add_argument("--ts-path", type=Path, default=TS_PATH)
    parser.add_argument("--py-path", type=Path, default=PY_PATH)
    args = parser.parse_args()
    hashes = collect_hashes(args.asset_dir)
    write_hashes(hashes, args.ts_path, args.py_path)
    print(f"Wrote CLI/GUI SHA-256 digests to {args.ts_path} and {args.py_path}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
