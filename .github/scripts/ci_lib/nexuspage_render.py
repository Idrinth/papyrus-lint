"""Renders a Nexus page's coverage markers from aggregated lcov reports.

Extracted out of render_nexuspage.py (which mixed this logic with its CLI
entrypoint) with no behavior change.
"""

from pathlib import Path

from .coverage_lcov import MODULES, iter_leaf_paths, parse_lcov

MARKERS = {
    "<COVERED_LINES>": "hit",
    "<TOTAL_LINES>": "found",
    "<COVERAGE_PERCENTAGE>": "percentage",
    "<VERSION>": "version",
    "<CONFIGURATION>": "configuration",
    "<CLI>": "cli",
}


def coverage_totals(artifacts: Path) -> tuple[int, int]:
    """Return (covered, total), requiring every CI coverage report."""
    hit = found = 0
    missing: list[str] = []
    for _, parts in MODULES:
        for relative_path in iter_leaf_paths(parts):
            result = parse_lcov(artifacts / relative_path)
            if result is None:
                missing.append(relative_path)
                continue
            report_found, report_hit, _report_functions_found = result
            found += report_found
            hit += report_hit

    if missing:
        raise ValueError("missing coverage reports: " + ", ".join(missing))
    if found == 0:
        raise ValueError("coverage reports contain no lines")
    return hit, found


def render(template: str, hit: int, found: int, version: str) -> str:
    """Replace each expected marker exactly once."""
    values = {
        "hit": f"{hit:,}",
        "found": f"{found:,}",
        "percentage": f"{hit / found * 100:.1f}",
        "version": version,
        "configuration": Path("docs/papyrus-lint.default.yaml").read_text(encoding="utf-8", errors="replace"),
        "cli": Path("docs/papyrus-cli-usage.txt").read_text(encoding="utf-8", errors="replace"),
    }
    rendered = template
    for marker, value_name in MARKERS.items():
        rendered = rendered.replace(marker, values[value_name])
    return rendered
