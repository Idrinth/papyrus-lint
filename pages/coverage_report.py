"""Coverage report parsing, formatting, and the coverage.html subpage
itself for the Pages builder.

Loads .github/scripts/coverage_summary.py by file path (see
load_coverage_summary) so the breakdown reuses its MODULES grouping and
pct() formatting rather than duplicating them, and can never drift from
the coverage figures already shown in release notes and PR comments.
parse_lcov_files/normalize_source_path add the per-file granularity
coverage_summary.py itself doesn't need, by parsing a directory of
downloaded lcov.info reports directly and stripping a CI runner's
absolute checkout prefix off each lcov SF: path so files display
relative to the repository root. build_coverage_page (extracted out of
pages/build.py, which was getting long and crowded with unrelated
site-assembly concerns, with no behavior change) then assembles that
breakdown into coverage.html, falling back to a "data unavailable"
placeholder when no --coverage-dir was passed to the build.
"""

from __future__ import annotations

import html
import importlib.util
import sys
from pathlib import Path

try:
    from pages.site_chrome import finalize_page, render_shared_components
except ImportError:  # running as pages/build.py
    from site_chrome import finalize_page, render_shared_components

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent

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
    MODULES grouping and pct() formatting instead of duplicating them.

    coverage_summary.py itself imports from the ci_lib package that lives
    alongside it, so .github/scripts (ci_lib's parent) must be on sys.path
    for that import to resolve, since exec_module doesn't add it there
    automatically the way running the script directly would."""
    scripts_dir = str(COVERAGE_SUMMARY_SCRIPT.parent)
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)
    spec = importlib.util.spec_from_file_location("coverage_summary", COVERAGE_SUMMARY_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def normalize_source_path(raw_path: str, artifact_name: str = "") -> str:
    """Strips a CI runner's absolute checkout prefix off an lcov SF: path, so
    the coverage subpage displays paths relative to the repository root the
    same way the rest of the site links into it. Relative frontend and VS Code
    extension paths are rooted using the artifact that supplied the report."""
    normalized = raw_path.strip().replace("\\", "/")
    marker_at = normalized.rfind(REPO_CHECKOUT_MARKER)
    if marker_at != -1:
        normalized = normalized[marker_at + len(REPO_CHECKOUT_MARKER) :]

    # JavaScript coverage tools report paths relative to their working or
    # compiled-output directory. Restore the repository-relative roots used
    # by the source tree before displaying them on the site.
    if artifact_name == "frontend-coverage" and not normalized.startswith("app/"):
        normalized = f"app/{normalized}"
    elif artifact_name == "vscode-extension-coverage":
        if normalized.startswith("vscode-extension/out-test/"):
            normalized = normalized.removeprefix("vscode-extension/out-test/")
        elif normalized.startswith("out-test/"):
            normalized = normalized.removeprefix("out-test/")
        elif normalized.startswith("vscode-extension/"):
            return normalized
        normalized = f"vscode-extension/{normalized}"
    return normalized


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
            current_file = normalize_source_path(line[3:], path.parent.name)
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


def build_coverage_page(out_dir: Path, coverage_dir: Path | None, version: str) -> None:
    template = (PAGES_DIR / "coverage.template.html").read_text(encoding="utf-8")
    if "<!--COVERAGE_CONTENT-->" not in template:
        raise SystemExit("coverage.template.html: missing marker <!--COVERAGE_CONTENT-->")
    if coverage_dir is not None and coverage_dir.is_dir():
        content = build_coverage_content(coverage_dir, load_coverage_summary())
    else:
        content = '<p class="section-intro">Coverage data isn\'t available for this build.</p>'
    page = template.replace("<!--COVERAGE_CONTENT-->", content)
    page = page.replace("<!--COVERAGE_VERSION-->", html.escape(version) if version else "unreleased")
    page = render_shared_components(page, "", version)
    (out_dir / "coverage.html").write_text(finalize_page(page), encoding="utf-8")
