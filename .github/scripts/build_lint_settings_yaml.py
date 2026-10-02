#!/usr/bin/env python3
"""Optional debug dump of the per-setting JSON files into one YAML document.

Builds read shared/configuration/lint-settings/*.json and
shared/configuration/lint-settings.yaml directly. This file is git-ignored
and is not a build input.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from ci_lib.lint_settings_yaml import assemble_lint_settings, render_lint_settings_yaml


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--project-settings-dir",
        type=Path,
        default=Path("shared/configuration/project-settings"),
        help="directory of project-settings/<key>.json files",
    )
    parser.add_argument(
        "--lint-settings-dir",
        type=Path,
        default=Path("shared/configuration/lint-settings"),
        help="directory of lint-settings/<key>.json files",
    )
    parser.add_argument(
        "--meta",
        type=Path,
        default=Path("shared/configuration/lint-settings.yaml"),
        help="declaration order and rules_comment",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("shared/configuration/lint-settings.generated.yaml"),
        help="path to write the combined YAML to",
    )
    args = parser.parse_args()

    document = assemble_lint_settings(args.project_settings_dir, args.lint_settings_dir, args.meta)
    args.out.write_text(render_lint_settings_yaml(document), encoding="utf-8")
    print(
        f"Wrote {len(document['project'])} project settings and "
        f"{len(document['settings'])} lint settings to {args.out}."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
