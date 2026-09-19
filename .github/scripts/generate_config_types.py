#!/usr/bin/env python3
"""Generate app/src/config-types.ts from rule metadata and the default YAML.

The type/default-assembly logic lives in ci_lib/config_types.py; this is
just the CLI entrypoint. Run from the repo root or via `npm run
generate:config-types` in `app/` before a frontend typecheck, test, or
dev server — the file is git-ignored, not checked in, the same way
`papyrus-lints/build.rs` writes `Rules` into `$OUT_DIR` instead of
committing it.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.config_types import write_config_types
from ci_lib.rules_json import assemble_rules

REPO_ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--rules-dir",
        type=Path,
        default=REPO_ROOT / "shared" / "rules",
        help="directory of shared/rules/<id>.json files",
    )
    parser.add_argument(
        "--default-yaml",
        type=Path,
        default=REPO_ROOT / "configuration" / "papyrus-lint.default.yaml",
        help="path to configuration/papyrus-lint.default.yaml",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=REPO_ROOT / "app" / "src" / "config-types.ts",
        help="path to write the generated TypeScript to",
    )
    args = parser.parse_args()

    rules = assemble_rules(args.rules_dir)
    default_yaml = args.default_yaml.read_text(encoding="utf-8")
    write_config_types(rules, default_yaml, args.out)
    print(f"Wrote {len(rules)} rule flags to {args.out}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
