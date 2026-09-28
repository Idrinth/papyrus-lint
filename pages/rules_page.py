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
# Built-in configuration presets from papyrus-lint-config (strict = default
# yaml; standard/careful drop "low" importance rules, except standard keeps
# the handful marked kept_in_standard). "none" is the filter value for a
# rule that is off in every built-in preset (enabled_by_default: false).
RULE_PRESETS = ["strict", "standard", "careful"]
NONE_PRESET = "none"


def load_rules() -> list[dict]:
    return json.loads(RULES_FILE.read_text(encoding="utf-8"))


def presets_for(rule: dict) -> list[str]:
    """Which built-in presets enable `rule`, matching papyrus-lint-config's
    `preset_rule_value`: a rule already off by default stays off everywhere;
    `strict` keeps the default; `careful` turns off every \"low\" importance
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
    """Renders the search box and severity/tag/auto-fix checkboxes above the
    rules table. Every control degrades to a no-op without rules.js: all
    checkboxes start checked, so every row is already visible; applyFilters()
    only ever narrows that starting set."""
    severities = [severity for severity in RULE_SEVERITIES if any(rule["severity"] == severity for rule in rules)]
    tags = [tag for tag in RULE_TAGS if any(tag in rule["tags"] for tag in tags)]
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
