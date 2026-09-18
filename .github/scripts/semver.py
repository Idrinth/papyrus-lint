"""Recommends the next semantic version to tag, based on the `type: *` labels
carried by the pull requests merged since the latest release.

Extracted out of semver_advisory.py (which mixed this logic with its CLI
entrypoint) with no behavior change. A pull request whose `component: *`
labels are exclusively among CI, pages, and documentation always recommends
a patch bump instead, regardless of its `type: *` label(s) (if any): none of
those components reach the end user, so they can never justify a major or
minor bump.
"""

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

MENTION_RE = re.compile(r"@([A-Za-z0-9](?:-?[A-Za-z0-9/])*)")


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


def escape_mentions(text: str) -> str:
    """Wraps any "@name"/"@org/team" mention in backticks so GitHub renders
    it as literal text in the release body instead of turning it into a
    user/team mention (and notifying them)."""
    return MENTION_RE.sub(lambda match: f"`{match.group(0)}`", text)


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
        lines.append(f"- #{pr.get('number')} {escape_mentions(pr.get('title', ''))}")

    if coverage_summary and coverage_summary.strip():
        lines.append("")
        lines.append("### Test coverage")
        lines.append("")
        lines.append(coverage_summary.strip())

    return "\n".join(lines) + "\n"
