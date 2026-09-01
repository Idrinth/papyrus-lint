#!/usr/bin/env python3
"""Aggregates lcov.info coverage reports by project module and renders a
Markdown summary for the CI coverage PR comment.

Usage: coverage_summary.py <dir-containing-downloaded-artifacts>
"""

import sys
from pathlib import Path

MARKER = "<!-- coverage-summary-comment -->"

# Each module maps to a display label and the lcov.info files (relative to
# the downloaded-artifacts directory) that make it up.
MODULES = [
    (
        "Crates (papyrus-parser, papyrus-lints, papyrus-lint-core, papyrus-lint-cli)",
        [
            ("papyrus-parser", "rust-coverage-papyrus-parser/lcov.info"),
            ("papyrus-lints", "rust-coverage-papyrus-lints/lcov.info"),
            ("papyrus-lint-core", "rust-coverage-papyrus-lint-core/lcov.info"),
            ("papyrus-lint-cli", "rust-coverage-papyrus-lint-cli/lcov.info"),
        ],
    ),
    (
        "App (src-tauri)",
        [("src-tauri", "rust-coverage-src-tauri/lcov.info")],
    ),
    (
        "UI (frontend)",
        [("frontend", "frontend-coverage/lcov.info")],
    ),
    (
        "Editor plugins",
        [
            ("vscode-extension", "vscode-extension-coverage/lcov.info"),
            ("SublimeLinter-contrib-papyrus-lint", "sublime-extension-coverage/lcov.info"),
        ],
    ),
]


def parse_lcov(path: Path) -> tuple[int, int] | None:
    """Returns (lines_found, lines_hit) summed across every record in an
    lcov.info file, or None if the file doesn't exist."""
    if not path.is_file():
        return None
    found = hit = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("LF:"):
            found += int(line[3:])
        elif line.startswith("LH:"):
            hit += int(line[3:])
    return found, hit


def pct(hit: int, found: int) -> str:
    if found == 0:
        return "n/a"
    return f"{hit / found * 100:.1f}%"


def main() -> None:
    root = Path(sys.argv[1])

    lines = [MARKER, "### Coverage by module", "", "| Module | Coverage | Lines covered |", "| --- | --- | --- |"]
    total_found = total_hit = 0
    any_report = False

    for label, parts in MODULES:
        module_found = module_hit = 0
        any_missing = False
        part_rows = []
        for name, rel_path in parts:
            result = parse_lcov(root / rel_path)
            if result is None:
                any_missing = True
                part_rows.append(f"| ↳ {name} | _no report_ | |")
                continue
            found, hit = result
            any_report = True
            module_found += found
            module_hit += hit
            total_found += found
            total_hit += hit
            part_rows.append(f"| ↳ {name} | {pct(hit, found)} | {hit}/{found} |")

        summary = pct(module_hit, module_found)
        if any_missing and module_found == 0:
            summary = "n/a"
        lines.append(f"| {label} | {summary} | {module_hit}/{module_found} |")

        if len(parts) > 1:
            lines.extend(part_rows)

    total_summary = pct(total_hit, total_found) if any_report else "n/a"
    lines.append(f"| **Total** | **{total_summary}** | **{total_hit}/{total_found}** |")

    lines.append("")
    lines.append("_Line coverage, aggregated from each job's lcov report. Missing reports mean that job didn't run or didn't upload one._")

    print("\n".join(lines))


if __name__ == "__main__":
    main()
