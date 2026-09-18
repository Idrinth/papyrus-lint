#!/usr/bin/env python3
"""Render a Nexus page with aggregate line-coverage values.

The rendering logic lives in nexuspage_render.py; this is just the CLI
entrypoint.
"""

import sys
from pathlib import Path

from nexuspage_render import coverage_totals, render


def main() -> None:
    if len(sys.argv) != 5:
        raise SystemExit(
            "usage: render_nexuspage.py TEMPLATE COVERAGE_ARTIFACTS OUTPUT VERSION"
        )

    template_path, artifacts_path, output_path = map(Path, sys.argv[1:4])
    version = sys.argv[4]
    hit, found = coverage_totals(artifacts_path)
    output_path.write_text(
        render(template_path.read_text(encoding="utf-8"), hit, found, version),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
