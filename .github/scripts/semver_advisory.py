#!/usr/bin/env python3
"""Recommends the next semantic version to tag, based on the `type: *` labels
carried by the pull requests merged since the latest release (see the
`semver-advisory` CI job, which gathers those pull requests via the GitHub
API and passes them to this script as a JSON file). A pull request whose
`component: *` labels are exclusively among CI, pages, and documentation
always recommends a patch bump instead, regardless of its `type: *` label(s)
(if any): none of those components reach the end user, so they can never
justify a major or minor bump. This script itself never creates a tag or
changes any file; the `semver-advisory` job uses its output to create or
update a draft GitHub release for the recommended version, which a
maintainer can edit and publish (or leave alone) whenever they actually
want to cut that release.

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
import re

BREAKING_LABEL = "type: breaking change"
FEATURE_LABEL = "type: feature"
PATCH_LABELS = {"type: refactoring", "type: tests", "type: documentation", "type: dependency"}

# Components that never reach the end user: a pull request touching only
# these (per its `component: *` labels) is always a patch-level change,
# whatever `type: *` label(s) it also carries.
NON_USER_FACING_COMPONENTS = {"component: ci", "component: pages", "component: documentation"}

BUMP_RANK = {"major": 0, "minor": 1, "patch": 2}

TAG_RE = re.compile(r"^v(\d+)\.(\d+)\.(\d+)$")

# Embedded in every draft release this job creates, so the workflow can tell
# its own auto-generated draft apart from one a maintainer created by hand
# (which must be left untouched) when deciding whether to update or replace it.
RELEASE_MARKER = "<!-- semver-advisory: auto-generated draft -->"


def classify_pull_request(labels: list[str]) -> str | None:
    """The version-bump level ("major"/"minor"/"patch") implied by one pull
    request's labels, or None if it carries none of the recognized `type: *`
    labels. A pull request can carry more than one; breaking change beats
    feature beats the patch-level labels (refactoring/tests/documentation).
    A pull request whose `component: *` labels are all in
    NON_USER_FACING_COMPONENTS is always "patch", overriding that
    precedence, since none of those components affect the end user."""
    normalized = {label.strip().lower() for label in labels}
    components = {label for label in normalized if label.startswith("component: ")}
    if components and components <= NON_USER_FACING_COMPONENTS:
        return "patch"
    if BREAKING_LABEL in normalized:
        return "major"
    if FEATURE_LABEL in normalized:
        return "minor"
    if normalized & PATCH_LABELS:
        return "patch"
    return None


def recommend_bump(pull_requests: list[dict]) -> str | None:
    """The highest-precedence bump across every pull request, or None if none
    of them carry a recognized `type: *` label."""
    levels = {classify_pull_request(pr.get("labels", [])) for pr in pull_requests}
    levels.discard(None)
    if not levels:
        return None
    return min(levels, key=lambda level: BUMP_RANK[level])


def bump_version(current_tag: str | None, bump: str) -> str:
    match = TAG_RE.match(current_tag) if current_tag else None
    major, minor, patch = (int(part) for part in match.groups()) if match else (0, 0, 0)
    if bump == "major":
        return f"v{major + 1}.0.0"
    if bump == "minor":
        return f"v{major}.{minor + 1}.0"
    if bump == "patch":
        return f"v{major}.{minor}.{patch + 1}"
    raise ValueError(f"unknown bump level: {bump}")


def dedupe_pull_requests(pull_requests: list[dict]) -> list[dict]:
    """Keeps only the first entry seen for each pull request `number` (a pull
    request can show up once per commit it's associated with)."""
    seen: dict[int, dict] = {}
    for pr in pull_requests:
        number = pr.get("number")
        if number is None or number in seen:
            continue
        seen[number] = pr
    return list(seen.values())


def build_summary(
    current_tag: str | None, pull_requests: list[dict], bump: str | None, next_version: str | None
) -> str:
    baseline = current_tag or "(no previous release)"
    lines = ["### Semantic version advisory", "", f"Comparing against the latest release: `{baseline}`.", ""]

    if not pull_requests:
        lines.append("No merged pull requests found since the latest release.")
        return "\n".join(lines) + "\n"

    lines.append("| PR | Title | Recommended bump |")
    lines.append("| --- | --- | --- |")
    for pr in sorted(pull_requests, key=lambda pr: pr.get("number", 0)):
        level = classify_pull_request(pr.get("labels", [])) or "—"
        lines.append(f"| #{pr.get('number')} | {pr.get('title', '')} | {level} |")
    lines.append("")

    if bump is None:
        lines.append(
            "None of the merged pull requests carry a `type: *` label, so no version bump can be recommended."
        )
    else:
        lines.append(f"**Recommended next version: `{next_version}`** ({bump} bump).")

    return "\n".join(lines) + "\n"


def build_release_notes(
    pull_requests: list[dict],
    bump: str | None,
    next_version: str | None,
    coverage_summary: str | None = None,
) -> str:
    """Body for the draft release the calling workflow creates/updates for
    `next_version`. Kept deliberately simple (unlike the grouped-by-component
    changelist `release.yml` builds for an actual tagged release) since a
    real release replaces this draft's title/body once it's tagged; this is
    just enough for a maintainer reviewing the draft to see why that version
    was recommended, and how well tested the codebase currently is.
    `coverage_summary`, when non-blank, is folded in verbatim as a "Test
    coverage" section (the calling workflow builds it via
    `coverage_summary.py` against the latest successful CI run's lcov
    reports, the same way `release.yml` does for an actual tagged release).
    Empty when no bump is recommended, since then there's nothing to put in
    a draft release."""
    if bump is None or next_version is None:
        return ""

    lines = [
        RELEASE_MARKER,
        "",
        f"Auto-generated draft based on the `type: *` labels of the pull requests merged so far "
        f"({bump} bump). Review and edit before publishing — the actual release notes are "
        "regenerated when this version is tagged.",
        "",
        "### Pull requests",
        "",
    ]
    for pr in sorted(pull_requests, key=lambda pr: pr.get("number", 0)):
        lines.append(f"- #{pr.get('number')} {pr.get('title', '')}")

    if coverage_summary and coverage_summary.strip():
        lines.append("")
        lines.append("### Test coverage")
        lines.append("")
        lines.append(coverage_summary.strip())

    return "\n".join(lines) + "\n"


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
