#!/usr/bin/env python3
"""Recommends the next semantic version to tag (see the `semver-advisory` CI
job, which gathers pull requests merged since the latest release via the
GitHub API and passes them to this script).

The recommendation logic lives in ci_lib/semver.py; this is just the CLI
entrypoint.

Usage: semver_advisory.py <pull-requests.json> [current-tag]
           [--release-notes <path>] [--outputs <path>]
           [--coverage-summary <path>]

`pull-requests.json` is a JSON array of `{"number": ..., "title": ...,
"labels": [...]}` objects, one per pull request merged since `current-tag`
(the latest release tag, e.g. "v1.2.3"; omitted or empty when the project
has no release yet). Duplicate `number`s (a pull request can be associated
with more than one commit in the range) are collapsed to a single row.

The advisory itself is always printed to stdout. `--release-notes` also
writes the body for the draft release the calling workflow creates/updates
for the recommended version (empty when no bump is recommended).
`--outputs` also writes `{"bump": ..., "next_version": ...}` as JSON (both
`null` when no bump is recommended), so the calling workflow can decide
whether a draft release is needed without re-parsing the advisory text.
`--coverage-summary` points at a Markdown file (built by the calling
workflow via `coverage_summary.py`, the same as `release.yml`'s own
`release-notes` job) to fold into the draft release body as a "Test
coverage" section, so a maintainer reviewing the draft can see how well
tested the codebase is without leaving GitHub. Omitted, or the file is
missing/empty, just means the section is left out.
"""

import argparse
import json

from ci_lib.semver import build_release_notes, build_summary, bump_version, dedupe_pull_requests, recommend_bump


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pull_requests_path")
    parser.add_argument("current_tag", nargs="?", default=None)
    parser.add_argument(
        "--release-notes",
        dest="release_notes_path",
        default=None,
        help="Write the draft release body for the recommended version to this path.",
    )
    parser.add_argument(
        "--outputs",
        dest="outputs_path",
        default=None,
        help='Write {"bump": ..., "next_version": ...} as JSON to this path.',
    )
    parser.add_argument(
        "--coverage-summary",
        dest="coverage_summary_path",
        default=None,
        help="Path to a Markdown coverage summary (see coverage_summary.py) to fold into the "
        "draft release notes as a Test coverage section.",
    )
    args = parser.parse_args()

    current_tag = args.current_tag or None

    with open(args.pull_requests_path, encoding="utf-8") as handle:
        pull_requests = dedupe_pull_requests(json.load(handle))

    bump = recommend_bump(pull_requests)
    next_version = bump_version(current_tag, bump) if bump else None

    print(build_summary(current_tag, pull_requests, bump, next_version))

    if args.release_notes_path:
        coverage_summary = None
        if args.coverage_summary_path:
            try:
                with open(args.coverage_summary_path, encoding="utf-8") as handle:
                    coverage_summary = handle.read()
            except FileNotFoundError:
                coverage_summary = None
        with open(args.release_notes_path, "w", encoding="utf-8") as handle:
            handle.write(build_release_notes(pull_requests, bump, next_version, coverage_summary))

    if args.outputs_path:
        with open(args.outputs_path, "w", encoding="utf-8") as handle:
            json.dump({"bump": bump, "next_version": next_version}, handle)


if __name__ == "__main__":
    main()
