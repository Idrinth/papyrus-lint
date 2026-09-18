"""Assembles shared/rules.json from shared/rules/<id>.json.

shared/rules.json grew too large to review as one file, so each rule now
lives in its own shared/rules/<id>.json (a single rule object). This module
reassembles the combined array the Rust build scripts
(papyrus-lints/build.rs, papyrus-lint-config/build.rs), pages/rules_page.py,
and .github/scripts/generate_nexuspage_tables.py all still read from one
shared/rules.json file, so none of those consumers needed to change.
"""

from __future__ import annotations

import json
from pathlib import Path


def assemble_rules(rules_dir: Path) -> list[dict]:
    """Reads every *.json file directly under *rules_dir*, one rule object
    each, and returns them sorted by id (i.e. by file name), so the combined
    array's order is stable regardless of directory listing order.

    Each file's own `id` field must match its file name (minus `.json`),
    catching a rule renamed on one side but not the other.
    """
    paths = sorted(rules_dir.glob("*.json"))
    if not paths:
        raise ValueError(f"no rule files found in {rules_dir}")

    rules = []
    for path in paths:
        rule = json.loads(path.read_text(encoding="utf-8"))
        if rule.get("id") != path.stem:
            raise ValueError(f"{path}: `id` is {rule.get('id')!r}, expected {path.stem!r} to match the file name")
        rules.append(rule)
    return rules


def render_rules_json(rules: list[dict]) -> str:
    """Renders *rules* the same way the checked-in shared/rules.json used
    to be hand-formatted: a top-level JSON array, 2-space indent, with a
    trailing newline."""
    return json.dumps(rules, indent=2) + "\n"
