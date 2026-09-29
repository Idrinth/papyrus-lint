#!/usr/bin/env python3
"""Generate shared/configuration/papyrus-lint.default.yaml (and the config JSON Schema).

Run after editing a shared/configuration/project-settings or lint-settings
JSON file, or shared/rules/*.json (and after build_rules_json.py). This
refreshes the git-ignored lint-settings.yaml from those JSON files before
reading it. Both outputs are build/release/docs artifacts and are
git-ignored - not a source of truth. The schema is also produced so Pages and
CI jobs that already call this script can publish/validate without a checked-in
copy of schema/papyrus-lint.schema.json.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

from ci_lib.config_schema import write_schema  # noqa: E402
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
        help="Default YAML output path (default: shared/configuration/papyrus-lint.default.yaml)",
    )
    parser.add_argument(
        "--skip-schema",
        action="store_true",
        help="Do not also write schema/papyrus-lint.schema.json",
    )
    args = parser.parse_args()
    root = args.repo_root.resolve()
    dest = write_default_yaml(root, args.output)
    print(f"Wrote {dest}")
    if not args.skip_schema:
        schema_dest = write_schema(root)
        print(f"Wrote {schema_dest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
