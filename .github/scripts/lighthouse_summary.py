#!/usr/bin/env python3
"""Prints a Markdown Lighthouse report summary for the PR comment.

The summary-building logic lives in lighthouse_report.py; this is just the
CLI entrypoint.

Usage: lighthouse_summary.py <dir-of-report.json-files> [label]

`label`, when given, distinguishes this summary's comment/title from
another surface's (e.g. "App" for the app frontend vs. the default,
unlabeled comment used for the GitHub Pages site), so the two can coexist
as separate PR comments instead of overwriting each other.
"""

import sys
from pathlib import Path

from lighthouse_report import MARKER, build_summary, load_reports


def main() -> None:
    directory = Path(sys.argv[1])
    label = sys.argv[2] if len(sys.argv) > 2 else None
    marker = f"<!-- lighthouse-summary-comment-{label} -->" if label else MARKER
    title = f"{label} Lighthouse report" if label else "Lighthouse report"

    print(build_summary(load_reports(directory), marker=marker, title=title))


if __name__ == "__main__":
    main()
