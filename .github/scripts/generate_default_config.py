#!/usr/bin/env python3
"""Generate shared/configuration/papyrus-lint.default.yaml from lint-settings + rules.

Run after editing shared/configuration/lint-settings.yaml or shared/rules/*.json
(and after build_rules_json.py). The output is a build/release/docs artifact and
is git-ignored — not a source of truth.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from ci_lib.default_config import write_default_yaml  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=SCRIPT_DIR.parent.parent,
        help="Repository root (default: two levels above this script)",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=None,
        help="Output path (default: shared/configuration/papyrus-lint.default.yaml)",
    )
    args = parser.parse_args()
    dest = write_default_yaml(args.repo_root.resolve(), args.output)
    print(f"Wrote {dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
