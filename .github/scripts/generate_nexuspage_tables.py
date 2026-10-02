#!/usr/bin/env python3
"""Regenerate a Nexus page's lint tables from rule metadata.

The table-generation logic lives in ci_lib/nexuspage_tables.py; this is
just the CLI entrypoint. Pass either shared/rules (a directory of
<id>.json files) or a combined rules JSON array.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from ci_lib.nexuspage_tables import apply, render_tables
from ci_lib.rules_json import assemble_rules


def load_rules(path: Path) -> list[dict]:
    if path.is_dir():
        return assemble_rules(path)
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, list):
        raise ValueError(f"{path} must contain a JSON array")
    return loaded


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "rules",
        type=Path,
        help="path to shared/rules or a combined rules JSON array",
    )
    parser.add_argument("bbcode_file", type=Path, help="path to the Nexus page BBCode source")
    args = parser.parse_args()

    rules = load_rules(args.rules)
    original = args.bbcode_file.read_text(encoding="utf-8")
    updated = apply(original, render_tables(rules))

    args.bbcode_file.write_text(updated, encoding="utf-8")
    print(f"Regenerated lint tables in {args.bbcode_file} from {args.rules}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
