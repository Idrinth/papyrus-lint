#!/usr/bin/env python3
"""Regenerate a Nexus page's lint tables from its rules.json.

The table-generation logic lives in ci_lib/nexuspage_tables.py; this is
just the CLI entrypoint.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from ci_lib.nexuspage_tables import apply, render_tables


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
