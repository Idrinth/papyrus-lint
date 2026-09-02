#!/usr/bin/env python3
"""Render a Nexus page with aggregate line-coverage values."""

import sys
from pathlib import Path

from coverage_summary import MODULES, parse_lcov

MARKERS = {
    "<COVERED_LINES>": "hit",
    "<TOTAL_LINES>": "found",
    "<COVERAGE_PERCENTAGE>": "percentage",
}


def coverage_totals(artifacts: Path) -> tuple[int, int]:
    """Return (covered, total), requiring every CI coverage report."""
    hit = found = 0
    missing: list[str] = []
    for _, parts in MODULES:
        for _, relative_path in parts:
            result = parse_lcov(artifacts / relative_path)
            if result is None:
                missing.append(relative_path)
                continue
            report_found, report_hit = result
            found += report_found
            hit += report_hit

    if missing:
        raise ValueError("missing coverage reports: " + ", ".join(missing))
    if found == 0:
        raise ValueError("coverage reports contain no lines")
    return hit, found


def render(template: str, hit: int, found: int) -> str:
    """Replace each expected marker exactly once."""
    values = {
        "hit": str(hit),
        "found": str(found),
        "percentage": f"{hit / found * 100:.1f}",
    }
    rendered = template
    for marker, value_name in MARKERS.items():
        count = rendered.count(marker)
        if count != 1:
            raise ValueError(f"expected exactly one {marker} marker, found {count}")
        rendered = rendered.replace(marker, values[value_name])
    return rendered


def main() -> None:
    if len(sys.argv) != 4:
        raise SystemExit(
            "usage: render_nexuspage.py TEMPLATE COVERAGE_ARTIFACTS OUTPUT"
        )

    template_path, artifacts_path, output_path = map(Path, sys.argv[1:])
    hit, found = coverage_totals(artifacts_path)
    output_path.write_text(
        render(template_path.read_text(encoding="utf-8"), hit, found),
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
