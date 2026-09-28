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

RULES_FILE = SHARED_DIR / "rules.json"

RULE_SEVERITIES = ["error", "warning", "info"]
RULE_TAGS = ["correctness", "performance", "maintainability", "style"]
RULE_PRESETS = ["strict", "standard", "careful"]
NONE_PRESET = "none"


def load_rules() -> list[dict]:
    return json.loads(RULES_FILE.read_text(encoding="utf-8"))


def presets_for(rule: dict) -> list[str]:
    """Which built-in presets enable `rule`, matching papyrus-lint-config's
    `preset_rule_value`: a rule already off by default stays off everywhere;
    `strict` keeps the default; `careful` turns off every low-importance
    rule; `standard` does the same except for `kept_in_standard`."""
    if not rule.get("enabled_by_default", True):
        return []
    importance = rule.get("importance", "medium")
    kept_in_standard = bool(rule.get("kept_in_standard", False))
    names = ["strict"]
    if importance != "low" or kept_in_standard:
        names.append("standard")
    if importance != "low":
        names.append("careful")
    return names


def render_rules_filter_bar(rules: list[dict]) -> str:
    """Renders the search box and severity/tag/preset/auto-fix checkboxes."""
    severities = [severity for severity in RULE_SEVERITIES if any(rule["severity"] == severity for rule in rules)]
    tags = [tag for tag in RULE_TAGS if any(tag in rule["tags"] for rule in rules)]
    used_presets = {name for rule in rules for name in presets_for(rule)}
    has_unpreset = any(not presets_for(rule) for rule in rules)
    preset_names = [name for name in RULE_PRESETS if name in used_presets]

    def chip(css_class: str, value: str, label: str) -> str:
        value_attr = html.escape(value, quote=True)
        return (
            f'<label class="filter-chip"><input type="checkbox" class="{css_class}" value="{value_attr}" '
            f'checked /> {html.escape(label)}</label>'
        )

    severity_chips = "\n".join(chip("rules-severity-filter", severity, severity.title()) for severity in severities)
    tag_chips = "\n".join(chip("rules-tag-filter", tag, tag.title()) for tag in tags)
    preset_chips = "\n".join(chip("rules-preset-filter", name, name) for name in preset_names)
    if has_unpreset:
        preset_chips = (preset_chips + "\n" if preset_chips else "") + chip(
            "rules-preset-filter", NONE_PRESET, "None"
        )
    preset_group = (
        f'<div class="rules-filter-group" role="group" aria-label="Filter by preset">{preset_chips}</div>'
        if preset_chips
        else ""
    )
    return (
        '<div class="rules-filter-bar">'
        '<div class="rules-filter-group">'
        '<label class="visually-hidden" for="rules-search">Search rules</label>'
        '<input type="search" id="rules-search" placeholder="Search by name, id, or description" '
        'autocomplete="off" />'
        "</div>"
        f'<div class="rules-filter-group" role="group" aria-label="Filter by severity">{severity_chips}</div>'
        f'<div class="rules-filter-group" role="group" aria-label="Filter by tag">{tag_chips}</div>'
        f"{preset_group}"
        '<label class="filter-chip"><input type="checkbox" id="rules-fixable-filter" /> Auto-fixable only</label>'
        "</div>"
        '<p class="rules-count" id="rules-count" aria-live="polite"></p>'
    )


def render_rules_table(rules: list[dict]) -> str:
    """Renders every shared/rules.json rule into one table."""
    out = [
        '<div class="lint-table-wrap">',
        '<table class="lint-table lint-rules-table" id="rules-table">',
        "<thead><tr>",
        "<th>Rule</th>",
        "<th>Severity</th>",
        "<th>Tags</th>",
        "<th>Presets</th>",
        "<th>Description</th>",
        "<th>Auto-fix</th>",
        "</tr></thead>",
        "<tbody>",
    ]
    for rule in rules:
        row_id = html.escape(f"rule-{rule['id']}", quote=True)
        tags_attr = html.escape(" ".join(rule["tags"]), quote=True)
        severity_attr = html.escape(rule["severity"], quote=True)
        preset_names = presets_for(rule)
        presets_attr = html.escape(" ".join(preset_names) if preset_names else NONE_PRESET, quote=True)
        search_text = " ".join(
            [rule["id"], rule["name"], rule["description"], *preset_names]
        ).lower()
        out.append(
            f'<tr id="{row_id}" data-severity="{severity_attr}" data-tags="{tags_attr}" '
            f'data-presets="{presets_attr}" '
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
        if preset_names:
            preset_badges = "".join(
                f'<span class="tag-badge preset-badge">{html.escape(name)}</span>' for name in preset_names
            )
        else:
            preset_badges = '<span class="tag-badge preset-badge preset-badge--none">none</span>'
        out.append(f"<td>{preset_badges}</td>")
        out.append(
            f"<td>{render_inline(rule['description'])}"
            f"<details><summary>Full behavior</summary><p>{render_inline(rule['definition'])}</p></details></td>"
        )
        out.append('<td class="fix-yes">✓</td>' if rule["fixable"] else "<td></td>")
        out.append("</tr>")
    out.append("</tbody></table></div>")
    return "\n".join(out)


def build_rules_page(out_dir: Path, version: str = "") -> None:
    """Renders shared/rules.json into rules.html."""
    rules = load_rules()
    template = (PAGES_DIR / "rules.template.html").read_text(encoding="utf-8")
    if "<!--RULES_CONTENT-->" not in template:
        raise SystemExit("rules.template.html: missing marker <!--RULES_CONTENT-->")
    content = render_rules_filter_bar(rules) + render_rules_table(rules)
    page = template.replace("<!--RULES_CONTENT-->", content)
    page = page.replace("<!--RULES_COUNT-->", str(len(rules)))
    page = render_shared_components(page, "", version)
    (out_dir / "rules.html").write_text(finalize_page(page), encoding="utf-8")
