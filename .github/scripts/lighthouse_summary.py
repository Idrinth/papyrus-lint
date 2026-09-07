#!/usr/bin/env python3
"""Aggregates a directory of Lighthouse JSON reports (see the
`pages-lighthouse`/`app-lighthouse` CI jobs, which run the `lighthouse` CLI
against every page of the built GitHub Pages site / app frontend) into a
Markdown summary for the PR comment: one row per page with its category
scores, plus a list of the specific audits behind any score that falls
under THRESHOLD.

Usage: lighthouse_summary.py <dir-of-report.json-files> [label]

`label`, when given, distinguishes this summary's comment/title from
another surface's (e.g. "App" for the app frontend vs. the default,
unlabeled comment used for the GitHub Pages site), so the two can coexist
as separate PR comments instead of overwriting each other.
"""

import json
import sys
from pathlib import Path

MARKER = "<!-- lighthouse-summary-comment -->"
THRESHOLD = 0.9

CATEGORIES = [
    ("performance", "Performance"),
    ("accessibility", "Accessibility"),
    ("best-practices", "Best Practices"),
    ("seo", "SEO"),
]


def page_label(report: dict) -> str:
    """The report's page, as a site-relative path (e.g. "/docs/index.html")."""
    url = report.get("finalUrl") or report.get("requestedUrl") or ""
    after_host = url.split("://", 1)[-1]
    parts = after_host.split("/", 1)
    path = parts[1] if len(parts) > 1 else ""
    return f"/{path}" if path else "/"


def format_score(score: float | None) -> str:
    if score is None:
        return "n/a"
    pct = round(score * 100)
    return f"{pct} ⚠️" if score < THRESHOLD else str(pct)


def active_categories(reports: list[dict]) -> list[tuple[str, str]]:
    """CATEGORIES filtered down to the ones actually present in `reports`, so a
    surface that runs Lighthouse with a narrower `--only-categories` (e.g. the
    app frontend, which skips SEO since it's never a crawled/indexed page)
    doesn't render a column of "n/a" for the category it deliberately omitted."""
    present = {key for report in reports for key in report.get("categories", {})}
    return [(key, label) for key, label in CATEGORIES if key in present]


def failing_audits(report: dict, categories_list: list[tuple[str, str]] = CATEGORIES) -> list[str]:
    """Titles of the audits behind every category below THRESHOLD, weakest first."""
    categories = report.get("categories", {})
    audits = report.get("audits", {})

    wanted_ids: set[str] = set()
    for key, _ in categories_list:
        category = categories.get(key)
        if not category or (category.get("score") if category.get("score") is not None else 1) >= THRESHOLD:
            continue
        for ref in category.get("auditRefs", []):
            if ref.get("weight", 0) > 0:
                wanted_ids.add(ref["id"])

    scored = []
    for audit_id in wanted_ids:
        audit = audits.get(audit_id)
        if not audit or audit.get("score") is None or audit["score"] >= THRESHOLD:
            continue
        scored.append((audit["score"], audit.get("title", audit_id)))

    scored.sort(key=lambda item: item[0])
    return [title for _, title in scored]


def build_summary(reports: list[dict], marker: str = MARKER, title: str = "Lighthouse report") -> str:
    lines = [marker, f"### {title}", ""]

    if not reports:
        lines.append("No Lighthouse reports were generated.")
        lines.append("")
        return "\n".join(lines)

    reports = sorted(reports, key=page_label)
    categories_list = active_categories(reports)

    header = " | ".join(label for _, label in categories_list)
    lines.append(f"| Page | {header} |")
    lines.append("| --- | " + " | ".join(["---"] * len(categories_list)) + " |")

    issue_sections = []
    for report in reports:
        label = page_label(report)
        categories = report.get("categories", {})
        cells = [format_score((categories.get(key) or {}).get("score")) for key, _ in categories_list]
        lines.append(f"| {label} | " + " | ".join(cells) + " |")

        issues = failing_audits(report, categories_list)
        if issues:
            bullets = "\n".join(f"  - {issue}" for issue in issues)
            issue_sections.append(f"- **{label}**\n{bullets}")

    lines.append("")
    if issue_sections:
        lines.append(f"Audits behind a category scoring below {int(THRESHOLD * 100)}/100:")
        lines.append("")
        lines.extend(issue_sections)
    else:
        lines.append(f"All pages scored at least {int(THRESHOLD * 100)}/100 in every category.")
    lines.append("")

    return "\n".join(lines)


def load_reports(directory: Path) -> list[dict]:
    if not directory.is_dir():
        return []
    return [json.loads(path.read_text(encoding="utf-8")) for path in sorted(directory.glob("*.report.json"))]


def main() -> None:
    directory = Path(sys.argv[1])
    label = sys.argv[2] if len(sys.argv) > 2 else None
    marker = f"<!-- lighthouse-summary-comment-{label} -->" if label else MARKER
    title = f"{label} Lighthouse report" if label else "Lighthouse report"

    print(build_summary(load_reports(directory), marker=marker, title=title))


if __name__ == "__main__":
    main()
