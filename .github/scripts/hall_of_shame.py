#!/usr/bin/env python3
"""Prints a Markdown hall of shame and GitHub Actions `notice` annotations so
the top offenders show up as Info messages on a pull request.

The ranking logic lives in ci_lib/source_metrics.py; this is just the CLI
entrypoint.

Usage: hall_of_shame.py [--root DIR] [--lcov-dir DIR] [--top N] [--no-notices]
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.source_metrics import build_rankings, emit_notices, render_report

DEFAULT_TOP = 3


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."), help="Repository root to scan")
    parser.add_argument(
        "--lcov-dir",
        type=Path,
        default=None,
        help="Directory containing downloaded lcov.info artifacts",
    )
    parser.add_argument("--top", type=int, default=DEFAULT_TOP, help="How many files to list per category")
    parser.add_argument(
        "--no-notices",
        action="store_true",
        help="Do not emit GitHub Actions notice annotations",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    root = args.root
    size_rows, export_rows, uncovered_rows, loc_rows = build_rankings(root, args.lcov_dir, args.top)
    print(render_report(size_rows, export_rows, uncovered_rows, loc_rows, args.top))
    if not args.no_notices:
        emit_notices(size_rows, export_rows, uncovered_rows, loc_rows)


if __name__ == "__main__":
    main()
