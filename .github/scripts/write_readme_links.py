#!/usr/bin/env python3
"""Bake shared/links.yaml's contact links into the editor plugin READMEs.

The rendering logic lives in ci_lib/readme_links.py; this is just the CLI
entrypoint.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.readme_links import README_PATHS, write_readme_links


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "readme",
        type=Path,
        nargs="*",
        default=list(README_PATHS),
        help="README.md path(s) to fill (default: the editor plugin READMEs)",
    )
    args = parser.parse_args()
    write_readme_links(args.readme)
    print("Wrote contact links to " + ", ".join(str(path) for path in args.readme) + ".")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
