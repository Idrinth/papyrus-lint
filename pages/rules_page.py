"""Rule table rendering for the rules.html subpage.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change. Renders
shared/rules.json's own metadata (id, severity, tags, auto-fix support, full
documented behavior) into a searchable/filterable reference of every lint
rule the linter implements.
"""

from __future__ import annotations

import html
import json
from pathlib import Path

try:
    from pages.markdown_render import render_inline
    from pages.site_chrome import finalize_page, render_shared_components
except ImportError:  # running as pages/build.py
    from markdown_render import render_inline
    from site_chrome import finalize_page, render_shared_components

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent
SHARED_DIR = ROOT / "shared"

# shared/rules.json's rule metadata, rendered onto rules.html by build_rules_page
# below - the single source of truth for every rule the linter implements, so
# a new rule needs no changes here at all.
RULES_FILE = SHARED_DIR / "rules.json"

# shared/rules.json's own richer rule metadata (id/severity/tags/fixable/full
# definition, one entry per lint) has no notion of README.md's five-category
# grouping, so rules.html instead lists every rule in one searchable/
# filterable table (see render_rules_table/build_rules_page below). Kept in
# display order so severity/tag filter checkboxes render in a stable,
# meaningful order rather than whatever order set iteration happens to give.
RULE_SEVERITIES = ["error", "warning", "info"]
RULE_TAGS = ["correctness", "performance", "maintainability", "style"]


def load_rules() -> list[dict]:
    return json.loads(RULES_FILE.read_text(encoding="utf-8"))


def render_rules_filter_bar(rules: list[dict]) -> str:
    """Renders the search box and severity/tag/auto-fix checkboxes above the
    rules table. Every control degrades to a no-op without rules.js: all
    checkboxes start checked, so every row is already visible; applyFilters()
    only ever narrows that starting set."""
    severities = [severity for severity in RULE_SEVERITIES if any(rule["severity"] == severity for rule in rules)]
    tags = [tag for tag in RULE_TAGS if any(tag in rule["tags"] for rule in rules)]

    def chip(css_class: str, value: str, label: str) -> str:
        value_attr = html.escape(value, quote=True)
        return (
            f'<label class="filter-chip"><input type="checkbox" class="{css_class}" value="{value_attr}" '
            f'checked /> {html.escape(label)}</label>'
        )

    severity_chips = "\n".join(chip("rules-severity-filter", severity, severity.title()) for severity in severities)
    tag_chips = "\n".join(chip("rules-tag-filter", tag, tag.title()) for tag in tags)
    return (
        '<div class="rules-filter-bar">'
        '<div class="rules-filter-group">'
        '<label class="visually-hidden" for="rules-search">Search rules</label>'
        '<input type="search" id="rules-search" placeholder="Search by name, id, or description" '
        'autocomplete="off" />'
        "</div>"
        f'<div class="rules-filter-group" role="group" aria-label="Filter by severity">{severity_chips}</div>'
        f'<div class="rules-filter-group" role="group" aria-label="Filter by tag">{tag_chips}</div>'
        '<label class="filter-chip"><input type="checkbox" id="rules-fixable-filter" /> Auto-fixable only</label>'
        "</div>"
        '<p class="rules-count" id="rules-count" aria-live="polite"></p>'
    )


def render_rules_table(rules: list[dict]) -> str:
    """Renders every shared/rules.json rule into one table, carrying its full
    metadata (severity, tags, id, full definition). Each row's data-*
    attributes are what rules.js filters against."""
    out = [
        '<div class="lint-table-wrap">',
        '<table class="lint-table lint-rules-table" id="rules-table">',
        "<thead><tr>",
        "<th>Rule</th>",
        "<th>Severity</th>",
        "<th>Tags</th>",
        "<th>Description</th>",
        "<th>Auto-fix</th>",
        "</tr></thead>",
        "<tbody>",
    ]
    for rule in rules:
        row_id = html.escape(f"rule-{rule['id']}", quote=True)
        tags_attr = html.escape(" ".join(rule["tags"]), quote=True)
        severity_attr = html.escape(rule["severity"], quote=True)
        search_text = " ".join([rule["id"], rule["name"], rule["description"]]).lower()
        out.append(
            f'<tr id="{row_id}" data-severity="{severity_attr}" data-tags="{tags_attr}" '
            f'data-fixable="{"true" if rule["fixable"] else "false"}" '
            f'data-search="{html.escape(search_text, quote=True)}">'
        )
        out.append(
            f'<td><a href="#{row_id}">{html.escape(rule["name"])}</a><br />'
            f'<code>{html.escape(rule["id"])}</code></td>'
        )
        out.append(f'<td><span class="severity-badge severity-badge--{severity_attr}">{rule["severity"]}</span></td>')
        tag_badges = "".join(f'<span class="tag-badge">{html.escape(tag)}</span>' for tag in rule["tags"])
        out.append(f"<td>{tag_badges}</td>")
        out.append(
            f"<td>{render_inline(rule['description'])}"
            f"<details><summary>Full behavior</summary><p>{render_inline(rule['definition'])}</p></details></td>"
        )
        out.append('<td class="fix-yes">✓</td>' if rule["fixable"] else "<td></td>")
        out.append("</tr>")
    out.append("</tbody></table></div>")
    return "\n".join(out)


def build_rules_page(out_dir: Path, version: str = "") -> None:
    """Renders shared/rules.json into rules.html (via rules.template.html), a
    full, searchable/filterable reference of every lint rule the linter
    implements, generated straight from the linter's own rule metadata so
    it carries each rule's id, severity, tags, and full documented behavior
    rather than just a short description."""
    rules = load_rules()
    template = (PAGES_DIR / "rules.template.html").read_text(encoding="utf-8")
    if "<!--RULES_CONTENT-->" not in template:
        raise SystemExit("rules.template.html: missing marker <!--RULES_CONTENT-->")
    content = render_rules_filter_bar(rules) + render_rules_table(rules)
    page = template.replace("<!--RULES_CONTENT-->", content)
    page = page.replace("<!--RULES_COUNT-->", str(len(rules)))
    page = render_shared_components(page, "", version)
    (out_dir / "rules.html").write_text(finalize_page(page), encoding="utf-8")
