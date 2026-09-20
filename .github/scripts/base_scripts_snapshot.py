#!/usr/bin/env python3
"""Run CLI preset output against unordered line-based baseline fixtures."""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

from ci_lib.base_scripts_snapshot import (
    FIXTURE_DIR,
    GAMES,
    PRESETS,
    SnapshotError,
    compare_output,
    fixture_path,
    render_output,
    write_fixture,
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
    parser.add_argument("--all", action="store_true", help="Run every built-in preset")
    parser.add_argument(
        "--update",
        action="store_true",
        help=f"Rewrite baselines under {FIXTURE_DIR} instead of comparing",
    )
    parser.add_argument(
        "--game",
        choices=GAMES,
        action="append",
        dest="games",
        help="The game(s) to run this for",
    )
    parser.add_argument(
        "--extender",
        action="store_true",
        help="The chosen game uses its script extender",
    )
    parser.add_argument(
        "--work-dir",
        type=Path,
        default=None,
        help="Directory used to unpack the zip and store CLI output (default: a temp dir)",
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


def selected_games(args: argparse.Namespace) -> tuple[str, ...]:
    if args.all or not args.games:
        return GAMES
    return tuple(dict.fromkeys(args.games))


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    root = args.root.resolve()
    cli = args.cli if args.cli.is_absolute() else (Path.cwd() / args.cli)

    def run(work_dir: Path) -> int:
        failed = False
        extenders = [args.extender]
        if args.all:
            extenders = [True, False]
        for extender in extenders:
            definer = "extended" if extender else "base"
            for preset in selected_presets(args):
                for game in selected_games(args):
                    print(f"linting {game} {definer} scripts with preset {preset}", flush=True)
                    try:
                        actual = render_output(
                            root,
                            cli,
                            preset,
                            work_dir / game / definer / preset,
                            game,
                            extender,
                        )
                    except SnapshotError as exc:
                        print(f"error ({preset}): {exc}", file=sys.stderr)
                        failed = True
                        continue
                    destination = fixture_path(root, preset, game, extender)
                    if args.update:
                        write_fixture(destination, actual)
                        print(f"wrote {destination.relative_to(root)}")
                        continue
                    diff = compare_output(actual, destination)
                    if diff is None:
                        print(f"ok ({preset}): matches {destination.relative_to(root)}")
                        continue
                    print(f"baseline mismatch ({preset})", file=sys.stderr)
                    print(_truncate_diff(diff), file=sys.stderr)
                    failed = True
        return 1 if failed else 0

    if args.work_dir is not None:
        args.work_dir.mkdir(parents=True, exist_ok=True)
        return run(args.work_dir)
    with tempfile.TemporaryDirectory(prefix="papyrus-lint-base-scripts-") as directory:
        return run(Path(directory))


if __name__ == "__main__":
    sys.exit(main())
