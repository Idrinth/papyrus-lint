#!/usr/bin/env python3
"""Check Nexus Mods BBCode for basic structural errors.

The actual linting logic lives in ci_lib/bbcode.py; this is just the CLI
entrypoint.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.bbcode import check_file


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+", type=Path)
    args = parser.parse_args()
    issue_count = sum(check_file(path) for path in args.files)
    if issue_count:
        print(f"BBCode lint failed with {issue_count} issue(s).")
        return 1
    print(f"BBCode lint passed for {len(args.files)} file(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
