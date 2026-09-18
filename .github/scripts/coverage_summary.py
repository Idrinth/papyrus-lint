#!/usr/bin/env python3
"""Renders a Markdown coverage-by-module summary for the CI coverage PR
comment, aggregating lcov.info reports via coverage_lcov.py.

Usage: coverage_summary.py <dir-containing-downloaded-artifacts>
"""

import sys
from pathlib import Path

from coverage_lcov import MODULES, crap_estimate, format_crap, pct, render_entry

MARKER = "<!-- coverage-summary-comment -->"


def main() -> None:
    root = Path(sys.argv[1])

    lines = [
        MARKER,
        "### Coverage by module",
        "",
        "| Module | Coverage | Lines covered | Est. CRAP |",
        "| --- | --- | --- | --- |",
    ]
    total_found = total_hit = total_functions_found = 0

    for label, parts in MODULES:
        module_found, module_hit, module_functions_found, rows = render_entry(root, label, parts, 0)
        total_found += module_found
        total_hit += module_hit
        total_functions_found += module_functions_found
        lines.extend(rows)

    total_summary = pct(total_hit, total_found)
    total_crap = format_crap(crap_estimate(total_found, total_hit, total_functions_found))
    lines.append(f"| **Total** | **{total_summary}** | **{total_hit}/{total_found}** | **{total_crap}** |")

    lines.append("")
    lines.append(
        "_Line coverage, aggregated from each job's lcov report. "
        "Missing reports mean that job didn't run or didn't upload one. "
        "Est. CRAP is a rough estimate — (avg. lines per function)² × (1 − line coverage)³ + "
        "avg. lines per function — standing in for a true cyclomatic-complexity CRAP score, "
        "which none of this project's coverage tools measure. Lower is better; treat it as a "
        "trend to watch, not a hard threshold._"
    )

    print("\n".join(lines))


if __name__ == "__main__":
    main()
