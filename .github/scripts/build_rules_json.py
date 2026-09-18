#!/usr/bin/env python3
"""Build the temporary shared/rules.json every other job's tooling reads.

The table-assembly logic lives in ci_lib/rules_json.py; this is just the
CLI entrypoint. Run before anything that reads shared/rules.json (a Rust
crate build, pages/build.py, its own test suite, or the Nexus page
generator) — it is git-ignored, not checked in.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.rules_json import assemble_rules, render_rules_json


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--rules-dir",
        type=Path,
        default=Path("shared/rules"),
        help="directory of shared/rules/<id>.json files (default: shared/rules)",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("shared/rules.json"),
        help="path to write the combined rules.json to (default: shared/rules.json)",
    )
    args = parser.parse_args()

    rules = assemble_rules(args.rules_dir)
    args.out.write_text(render_rules_json(rules), encoding="utf-8")
    print(f"Wrote {len(rules)} rules from {args.rules_dir} to {args.out}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
