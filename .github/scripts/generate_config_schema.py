#!/usr/bin/env python3
"""Generate or check schema/papyrus-lint.schema.json against lint-settings + rules."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from ci_lib.config_schema import render_schema, write_schema  # noqa: E402
from ci_lib.default_config import load_lint_settings, load_rules  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=SCRIPT_DIR.parent.parent)
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit non-zero if the checked-in schema drifts from generated output",
    )
    parser.add_argument("-o", "--output", type=Path, default=None)
    args = parser.parse_args()
    root = args.repo_root.resolve()
    if args.check:
        expected = render_schema(load_lint_settings(root), load_rules(root))
        path = args.output or (root / "schema" / "papyrus-lint.schema.json")
        actual = json.loads(path.read_text(encoding="utf-8"))
        if actual != expected:
            print(
                f"{path} is out of date; run python3 .github/scripts/generate_config_schema.py",
                file=sys.stderr,
            )
            return 1
        print(f"{path} matches lint-settings + rules")
        return 0
    dest = write_schema(root, args.output)
    print(f"Wrote {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
