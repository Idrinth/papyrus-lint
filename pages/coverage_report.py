"""Coverage report parsing and formatting for the coverage.html subpage.

Loads .github/scripts/coverage_summary.py by file path (see
load_coverage_summary) so the breakdown reuses its MODULES grouping and
pct() formatting rather than duplicating them, and can never drift from
the coverage figures already shown in release notes and PR comments.
parse_lcov_files/normalize_source_path add the per-file granularity
coverage_summary.py itself doesn't need, by parsing a directory of
downloaded lcov.info reports directly and stripping a CI runner's
absolute checkout prefix off each lcov SF: path so files display
relative to the repository root.
"""

from __future__ import annotations

import html
import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# .github/scripts isn't an importable package (its directory name starts
# with a dot), so this module loads it by file path instead. This reuses
# coverage_summary.py's MODULES grouping and lcov parsing rather than
# duplicating them, so the coverage subpage can never drift from the
# module breakdown used in the release notes and PR coverage comments.
COVERAGE_SUMMARY_SCRIPT = ROOT / ".github" / "scripts" / "coverage_summary.py"

# Matches a CI runner's absolute checkout prefix in an lcov SF: path (e.g.
# /home/runner/work/papyrus-lint/papyrus-lint/app/...), stripped off so the
# coverage subpage shows paths relative to the repository root.
REPO_CHECKOUT_MARKER = "/papyrus-lint/"


def load_coverage_summary():
    """Loads .github/scripts/coverage_summary.py by file path (see
    COVERAGE_SUMMARY_SCRIPT above) so the coverage subpage shares its
    MODULES grouping and pct() formatting instead of duplicating them."""
    spec = importlib.util.spec_from_file_location("coverage_summary", COVERAGE_SUMMARY_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def normalize_source_path(raw_path: str) -> str:
    """Strips a CI runner's absolute checkout prefix off an lcov SF: path, so
    the coverage subpage displays paths relative to the repository root the
    same way the rest of the site links into it. A path that's already
    relative (as some coverage tools emit) is returned unchanged."""
    normalized = raw_path.strip().replace("\\", "/")
    marker_at = normalized.rfind(REPO_CHECKOUT_MARKER)
    if marker_at == -1:
        return normalized
    return normalized[marker_at + len(REPO_CHECKOUT_MARKER) :]


def parse_lcov_files(path: Path) -> list[tuple[str, int, int]] | None:
    """Returns (source_file, lines_found, lines_hit) for every SF:/
    end_of_record record in an lcov.info file, or None if the file doesn't
    exist. The per-file counterpart of coverage_summary.parse_lcov, which
    only sums the whole report."""
    if not path.is_file():
        return None
    records: list[tuple[str, int, int]] = []
    current_file: str | None = None
    found = hit = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("SF:"):
            current_file = normalize_source_path(line[3:])
            found = hit = 0
        elif line.startswith("LF:"):
            found += int(line[3:])
        elif line.startswith("LH:"):
            hit += int(line[3:])
        elif line.startswith("end_of_record"):
            if current_file is not None:
                records.append((current_file, found, hit))
            current_file = None
    return records


def render_coverage_table(rows: list[tuple[str, int, int]], coverage_summary) -> str:
    if not rows:
        return '<p class="section-intro">No files reported.</p>'
    out = [
        '<div class="lint-table-wrap">',
        '<table class="lint-table">',
        "<thead><tr><th>File</th><th>Coverage</th><th>Lines</th></tr></thead>",
        "<tbody>",
    ]
    for name, found, hit in rows:
        out.append("<tr>")
        out.append(f"<td><code>{html.escape(name)}</code></td>")
        out.append(f"<td>{coverage_summary.pct(hit, found)}</td>")
        out.append(f"<td>{hit}/{found}</td>")
        out.append("</tr>")
    out.append("</tbody></table></div>")
    return "\n".join(out)


def render_coverage_entry(coverage_dir: Path, name: str, value, coverage_summary) -> tuple[int, int, bool, str]:
    """Renders one (name, value) MODULES entry's body (its own heading not
    included, since the caller decides whether to show it), recursing into
    nested groups the same way coverage_summary.render_entry does, since a
    value can be either a leaf lcov.info path or a further nested list of
    (label, value) entries (e.g. App's Crates). Returns (lines_found,
    lines_hit, whether any report was found, rendered body HTML)."""
    if isinstance(value, str):
        rows = parse_lcov_files(coverage_dir / value)
        if rows is None:
            return 0, 0, False, '<p class="section-intro">No report.</p>'
        found = sum(f for _, f, _h in rows)
        hit = sum(h for _, _f, h in rows)
        rows_sorted = sorted(rows, key=lambda r: ((r[2] / r[1]) if r[1] else 1.0, r[0]))
        return found, hit, True, render_coverage_table(rows_sorted, coverage_summary)

    found = hit = 0
    any_report = False
    child_html: list[str] = []
    for child_name, child_value in value:
        child_found, child_hit, child_any_report, child_body = render_coverage_entry(
            coverage_dir, child_name, child_value, coverage_summary
        )
        found += child_found
        hit += child_hit
        any_report = any_report or child_any_report
        heading = (
            f"<h3>{html.escape(child_name)} — {coverage_summary.pct(child_hit, child_found)} "
            f"({child_hit}/{child_found})</h3>"
        )
        child_html.append((heading if len(value) > 1 else "") + child_body)

    return found, hit, any_report, "".join(child_html)


def build_coverage_content(coverage_dir: Path, coverage_summary) -> str:
    """Renders the coverage subpage's body: a per-module, per-file line
    coverage breakdown from the downloaded lcov reports, worst-covered file
    first within each part so weak spots are immediately visible."""
    out: list[str] = []
    total_found = total_hit = 0
    any_report = False

    for label, parts in coverage_summary.MODULES:
        module_found = module_hit = 0
        part_html: list[str] = []
        for name, value in parts:
            found, hit, part_any_report, body = render_coverage_entry(coverage_dir, name, value, coverage_summary)
            module_found += found
            module_hit += hit
            total_found += found
            total_hit += hit
            any_report = any_report or part_any_report
            heading = (
                f"<h3>{html.escape(name)} — {coverage_summary.pct(hit, found)} ({hit}/{found})</h3>"
                if len(parts) > 1
                else ""
            )
            part_html.append(heading + body)

        summary = coverage_summary.pct(module_hit, module_found)
        out.append(
            f'<div class="coverage-module"><h2>{html.escape(label)} — {summary} '
            f"({module_hit}/{module_found})</h2>" + "".join(part_html) + "</div>"
        )

    total_summary = coverage_summary.pct(total_hit, total_found) if any_report else "n/a"
    out.insert(
        0,
        f'<p class="section-intro">Total line coverage: <strong>{total_summary}</strong> '
        f"({total_hit}/{total_found}).</p>",
    )
    return "\n".join(out)
