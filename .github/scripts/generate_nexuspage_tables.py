#!/usr/bin/env python3
"""Regenerate a Nexus page's lint tables from its rules.json.

Each `[spoiler][table]...[/table][/spoiler]` block in the BBCode file is
replaced, in order, with a table row per rule sharing that block's category
(the five categories, and their order, come from CATEGORIES below, matching
shared/rules.json's own `category` values) - sourced from that rule's `name`,
`description`, and `fixable` fields. Everything else in the file (headings,
intros, the configuration/CLI sections) is left untouched.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

CATEGORIES = ["Formatting", "Performance", "Reliability", "Bugprone", "Other"]

TABLE_RE = re.compile(r"\[spoiler\]\[table\]\[/table\]\[/spoiler\]", re.DOTALL)


def render_table(rules: list[dict]) -> str:
    """One [spoiler][table]...[/table][/spoiler] block for *rules*, in their
    given order, matching the header row and cell shape a Nexus lint table
    already uses."""
    rows = ["[tr][th]Lint[/th]\n[th]Description[/th]\n[th]Auto-Fix[/th]\n[/tr]\n"]
    for rule in rules:
        checkmark = "✓" if rule["fixable"] else ""
        rows.append(
            f"[tr][td][b]{rule['name']}[/b][/td]\n"
            f"[td]{rule['description']}[/td]\n"
            f"[td]{checkmark}[/td]\n"
            f"[/tr]\n"
        )
    return f"[spoiler][table]{''.join(rows)}[/table][/spoiler]"


def render_tables(rules: list[dict]) -> list[str]:
    """One rendered table per CATEGORIES entry, in that order."""
    by_category: dict[str, list[dict]] = {category: [] for category in CATEGORIES}
    for rule in rules:
        category = rule["category"]
        if category not in by_category:
            raise ValueError(f"{rule['id']}: unknown category {category!r}")
        by_category[category].append(rule)
    return [render_table(by_category[category]) for category in CATEGORIES]


def apply(text: str, tables: list[str]) -> str:
    """Replace each of *text*'s [spoiler][table]...[/table][/spoiler] blocks,
    in order, with the matching entry of *tables*."""
    matches = list(TABLE_RE.finditer(text))
    if len(matches) != len(tables):
        raise ValueError(f"expected {len(tables)} [spoiler][table] blocks, found {len(matches)}")

    pieces = []
    cursor = 0
    for match, table in zip(matches, tables, strict=True):
        pieces.append(text[cursor : match.start()])
        pieces.append(table)
        cursor = match.end()
    pieces.append(text[cursor:])
    return "".join(pieces)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rules_json", type=Path, help="path to rules.json")
    parser.add_argument("bbcode_file", type=Path, help="path to the Nexus page BBCode source")
    args = parser.parse_args()

    rules = json.loads(args.rules_json.read_text(encoding="utf-8"))
    original = args.bbcode_file.read_text(encoding="utf-8")
    updated = apply(original, render_tables(rules))

    args.bbcode_file.write_text(updated, encoding="utf-8")
    print(f"Regenerated lint tables in {args.bbcode_file} from {args.rules_json}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
