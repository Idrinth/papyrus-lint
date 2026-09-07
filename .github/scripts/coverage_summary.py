#!/usr/bin/env python3
"""Aggregates lcov.info coverage reports by project module and renders a
Markdown summary for the CI coverage PR comment.

Usage: coverage_summary.py <dir-containing-downloaded-artifacts>
"""

import sys
from pathlib import Path

MARKER = "<!-- coverage-summary-comment -->"

# Each module maps to a display label and either a single lcov.info file
# (relative to the downloaded-artifacts directory), or a nested list of
# further (label, ...) entries of the same shape, letting a module group
# sub-modules (e.g. App's Crates) arbitrarily deep. Groups and their
# entries are kept sorted alphabetically.
MODULES = [
    (
        "App",
        [
            (
                "Crates",
                [
                    ("papyrus-lint-cli", "rust-coverage-papyrus-lint-cli/lcov.info"),
                    ("papyrus-lint-core", "rust-coverage-papyrus-lint-core/lcov.info"),
                    ("papyrus-lints", "rust-coverage-papyrus-lints/lcov.info"),
                    ("papyrus-parser", "rust-coverage-papyrus-parser/lcov.info"),
                ],
            ),
            ("frontend", "frontend-coverage/lcov.info"),
            ("src-tauri", "rust-coverage-src-tauri/lcov.info"),
        ],
    ),
    (
        "Editor plugins",
        [
            ("SublimeLinter-contrib-papyrus-lint", "sublime-extension-coverage/lcov.info"),
            ("vscode-extension", "vscode-extension-coverage/lcov.info"),
        ],
    ),
    (
        "Tooling",
        [
            ("CI tooling", "ci-scripts-coverage/lcov.info"),
            ("Pages (site builder)", "pages-coverage/lcov.info"),
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


def iter_leaf_paths(entry):
    """Yields every lcov.info relative path nested (at any depth) under a
    MODULES entry's value: a leaf path string, or a further list of
    (label, value) entries."""
    if isinstance(entry, str):
        yield entry
        return
    for _, value in entry:
        yield from iter_leaf_paths(value)


def render_entry(root: Path, label: str, value, depth: int) -> tuple[int, int, list[str]]:
    """Renders one (label, value) MODULES entry, recursing into nested
    groups. Returns (lines_found, lines_hit, rendered_rows)."""
    prefix = "↳ " * depth
    if isinstance(value, str):
        result = parse_lcov(root / value)
        if result is None:
            return 0, 0, [f"| {prefix}{label} | _no report_ | |"]
        found, hit = result
        return found, hit, [f"| {prefix}{label} | {pct(hit, found)} | {hit}/{found} |"]

    found = hit = 0
    child_rows: list[str] = []
    for name, child in value:
        child_found, child_hit, rows = render_entry(root, name, child, depth + 1)
        found += child_found
        hit += child_hit
        child_rows.extend(rows)

    rows = [f"| {prefix}{label} | {pct(hit, found)} | {hit}/{found} |"]
    if len(value) > 1:
        rows.extend(child_rows)
    return found, hit, rows


def main() -> None:
    root = Path(sys.argv[1])

    lines = [MARKER, "### Coverage by module", "", "| Module | Coverage | Lines covered |", "| --- | --- | --- |"]
    total_found = total_hit = 0

    for label, parts in MODULES:
        module_found, module_hit, rows = render_entry(root, label, parts, 0)
        total_found += module_found
        total_hit += module_hit
        lines.extend(rows)

    total_summary = pct(total_hit, total_found)
    lines.append(f"| **Total** | **{total_summary}** | **{total_hit}/{total_found}** |")

    lines.append("")
    lines.append(
        "_Line coverage, aggregated from each job's lcov report. "
        "Missing reports mean that job didn't run or didn't upload one._"
    )

    print("\n".join(lines))


if __name__ == "__main__":
    main()
