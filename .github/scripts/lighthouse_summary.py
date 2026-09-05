#!/usr/bin/env python3
"""Builds a Markdown Lighthouse summary for the CI PR comment.

Reads the `manifest.json`-shaped list produced by `treosh/lighthouse-ci-action`'s
`manifest` output (one entry per collected page, each carrying category scores
under `summary` and a `jsonPath` pointing at that run's full Lighthouse report
on disk) and renders a per-page score table plus a list of failing audits.

Usage: lighthouse_summary.py <manifest.json>
"""

import json
import sys
from pathlib import Path

MARKER = "<!-- lighthouse-summary-comment -->"

# (summary/category key, display label), in report order.
CATEGORIES = [
    ("performance", "Performance"),
    ("accessibility", "Accessibility"),
    ("best-practices", "Best Practices"),
    ("seo", "SEO"),
]

# Audits scoring below this are called out individually as issues. Lighthouse
# scores are 0-1; audits without a numeric score (informational/manual ones)
# are never flagged.
ISSUE_THRESHOLD = 0.9


def score_str(value) -> str:
    if not isinstance(value, (int, float)):
        return "n/a"
    return str(round(value * 100))


def load_json(path: Path):
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None


def failing_audits(report: dict) -> list[str]:
    """Returns "Category: Audit title" for every scored audit below
    ISSUE_THRESHOLD in one of CATEGORIES, in category order."""
    found = []
    categories = report.get("categories", {})
    audits = report.get("audits", {})
    for category_key, category_label in CATEGORIES:
        category = categories.get(category_key)
        if not category:
            continue
        for ref in category.get("auditRefs", []):
            if ref.get("weight", 0) <= 0:
                continue
            audit = audits.get(ref.get("id", ""))
            if not audit:
                continue
            score = audit.get("score")
            if isinstance(score, (int, float)) and score < ISSUE_THRESHOLD:
                found.append(f"{category_label}: {audit.get('title', ref.get('id'))}")
    return found


def build_summary(manifest: list) -> str:
    lines = [MARKER, "### Lighthouse report", ""]

    runs = [entry for entry in manifest if entry.get("isRepresentativeRun")] or manifest
    if not runs:
        lines.append("_Lighthouse did not produce a report for this run._")
        return "\n".join(lines)

    header = "| Page | " + " | ".join(label for _, label in CATEGORIES) + " |"
    separator = "| --- | " + " | ".join("---" for _ in CATEGORIES) + " |"
    lines += [header, separator]

    issues_by_page = []
    for run in runs:
        page = run.get("url", "page")
        summary = run.get("summary") or {}
        scores = [score_str(summary.get(key)) for key, _ in CATEGORIES]
        lines.append(f"| {page} | " + " | ".join(scores) + " |")

        json_path = run.get("jsonPath")
        report = load_json(Path(json_path)) if json_path else None
        if report is None:
            continue

        issues = failing_audits(report)
        if issues:
            issues_by_page.append((page, issues))

    lines.append("")
    if issues_by_page:
        lines.append(f"#### Audits below {score_str(ISSUE_THRESHOLD)}")
        for page, issues in issues_by_page:
            lines.append(f"- {page}")
            for issue in issues:
                lines.append(f"  - {issue}")
    else:
        lines.append("No audits scored below the reporting threshold.")

    lines.append("")
    lines.append(
        "_Scores are 0-100 (higher is better). This check is informational and does not fail the build._"
    )

    return "\n".join(lines)


def main() -> None:
    manifest_path = Path(sys.argv[1])
    manifest = load_json(manifest_path)
    if manifest is None:
        manifest = []

    print(build_summary(manifest))


if __name__ == "__main__":
    main()
