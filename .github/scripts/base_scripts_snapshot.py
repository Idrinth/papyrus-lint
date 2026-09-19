#!/usr/bin/env python3
"""Compare PapyrusLinterCLI output on bundled Skyrim base scripts to snapshots.

Usage:
  base_scripts_snapshot.py --cli PATH --preset strict
  base_scripts_snapshot.py --cli PATH --all
  base_scripts_snapshot.py --cli PATH --all --update
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

from ci_lib.base_scripts_snapshot import (
    PRESETS,
    SNAPSHOT_DIR,
    SnapshotError,
    compare_snapshot,
    render_snapshot,
    snapshot_path,
    write_snapshot,
)


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True, help="Path to PapyrusLinterCLI")
    parser.add_argument("--root", type=Path, default=Path("."), help="Repository root")
    parser.add_argument(
        "--preset",
        choices=PRESETS,
        action="append",
        dest="presets",
        help="Built-in preset to run (repeatable). Defaults to all three when omitted.",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Run every built-in preset (strict, standard, careful)",
    )
    parser.add_argument(
        "--update",
        action="store_true",
        help=f"Rewrite snapshots under {SNAPSHOT_DIR} instead of comparing",
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        default=None,
        help="Directory used to unpack the zip and store CLI JSON (default: a temp dir)",
    )
    return parser.parse_args(argv)


DIFF_LOG_LIMIT = 200


def _truncate_diff(diff: str, limit: int = DIFF_LOG_LIMIT) -> str:
    lines = diff.splitlines()
    if len(lines) <= limit:
        return diff
    omitted = len(lines) - limit
    return "\n".join([*lines[:limit], f"... ({omitted} more diff lines omitted)"]) + "\n"


def selected_presets(args: argparse.Namespace) -> tuple[str, ...]:
    if args.all or not args.presets:
        return PRESETS
    return tuple(dict.fromkeys(args.presets))


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    root = args.root.resolve()
    cli = args.cli if args.cli.is_absolute() else (Path.cwd() / args.cli)
    failed = 0

    def run(work_dir: Path) -> int:
        nonlocal failed
        for preset in selected_presets(args):
            print(f"linting base scripts with preset {preset}", flush=True)
            try:
                actual = render_snapshot(root, cli, preset, work_dir / preset)
            except SnapshotError as exc:
                print(f"error ({preset}): {exc}", file=sys.stderr)
                failed += 1
                continue
            destination = snapshot_path(root, preset)
            if args.update:
                write_snapshot(destination, actual)
                print(f"wrote {destination.relative_to(root)}")
                continue
            diff = compare_snapshot(actual, destination)
            if diff is None:
                print(f"ok ({preset}): matches {destination.relative_to(root)}")
                continue
            print(f"snapshot mismatch ({preset})", file=sys.stderr)
            print(_truncate_diff(diff), file=sys.stderr)
            print(
                "Re-generate with "
                "`python3 .github/scripts/base_scripts_snapshot.py "
                f"--cli <PapyrusLinterCLI> --preset {preset} --update` "
                "after reviewing the delta.",
                file=sys.stderr,
            )
            failed += 1
        return 1 if failed else 0

    if args.work_dir is not None:
        args.work_dir.mkdir(parents=True, exist_ok=True)
        return run(args.work_dir)
    with tempfile.TemporaryDirectory(prefix="papyrus-lint-base-scripts-") as directory:
        return run(Path(directory))


if __name__ == "__main__":
    sys.exit(main())
