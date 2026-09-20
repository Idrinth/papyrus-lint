#!/usr/bin/env python3
"""Check that local Markdown links point to existing paths and anchors."""

from __future__ import annotations

import argparse
import html
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

MARKDOWN_SUFFIXES = {".md", ".markdown"}
SKIPPED_DIRECTORIES = {".git", "node_modules", "target"}
INLINE_LINK = re.compile(r"!?\[[^]]*\]\(\s*(<[^>]+>|[^\s)]+)")
REFERENCE_LINK = re.compile(r"^\s{0,3}\[[^]]+\]:\s*(<[^>]+>|\S+)")
FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
CODE_SPAN = re.compile(r"(`+)(.*?)\1")
ATX_HEADING = re.compile(r"^\s{0,3}#{1,6}\s+(.+?)\s*$")
SETEXT_HEADING = re.compile(r"^\s{0,3}(?:=+|-+)\s*$")
MARKDOWN_LINK_TEXT = re.compile(r"!?\[([^]]*)\]\([^)]*\)")
HTML_TAG = re.compile(r"<[^>]+>")
GITHUB_SLUG_PUNCTUATION = re.compile(r"[\u2000-\u206f\u2e00-\u2e7f\\'!\"#$%&()*+,./:;<=>?@[\]^`{|}~]")


def markdown_files(root: Path) -> list[Path]:
    """Return Markdown files below root, excluding generated/dependency trees."""
    if root.is_file():
        return [root] if root.suffix.lower() in MARKDOWN_SUFFIXES else []
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file()
        and path.suffix.lower() in MARKDOWN_SUFFIXES
        and not SKIPPED_DIRECTORIES.intersection(path.relative_to(root).parts)
    )


def local_target(destination: str) -> str | None:
    """Return the filesystem part of a local destination, or None if external."""
    destination = destination.removeprefix("<").removesuffix(">")
    parsed = urlsplit(destination)
    if parsed.scheme or parsed.netloc or destination.startswith(("#", "/")):
        return None
    return unquote(parsed.path) or None


def github_slug(heading: str) -> str:
    """Return the anchor GitHub generates for a Markdown heading."""
    heading = MARKDOWN_LINK_TEXT.sub(r"\1", heading)
    heading = HTML_TAG.sub("", heading)
    heading = html.unescape(heading).strip().lower()
    return re.sub(r"\s", "-", GITHUB_SLUG_PUNCTUATION.sub("", heading))


def anchors(path: Path) -> set[str]:
    """Return the GitHub-style heading anchors generated for a Markdown file."""
    result: set[str] = set()
    occurrences: dict[str, int] = {}
    fence_marker: str | None = None
    previous_line: str | None = None
    for line in path.read_text(encoding="utf-8").splitlines():
        fence = FENCE.match(line)
        if fence:
            marker = fence.group(1)[0]
            fence_marker = None if fence_marker == marker else marker
            previous_line = None
            continue
        if fence_marker:
            continue

        match = ATX_HEADING.match(line)
        heading = match.group(1).rstrip("#").rstrip() if match else None
        if SETEXT_HEADING.match(line) and previous_line and previous_line.strip():
            heading = previous_line.strip()
        if heading is not None:
            slug = github_slug(heading)
            duplicate = occurrences.get(slug, 0)
            candidate = slug if duplicate == 0 else f"{slug}-{duplicate}"
            while candidate in result:
                duplicate += 1
                candidate = f"{slug}-{duplicate}"
            occurrences[slug] = duplicate + 1
            result.add(candidate)
        previous_line = line
    return result


def broken_links(path: Path) -> list[tuple[int, str]]:
    """Return line numbers and destinations for nonexistent local links or anchors."""
    issues: list[tuple[int, str]] = []
    fence_marker: str | None = None
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        fence = FENCE.match(line)
        if fence:
            marker = fence.group(1)[0]
            fence_marker = None if fence_marker == marker else marker
            continue
        if fence_marker:
            continue

        line = CODE_SPAN.sub("", line)
        destinations = [match.group(1) for match in INLINE_LINK.finditer(line)]
        reference = REFERENCE_LINK.match(line)
        if reference:
            destinations.append(reference.group(1))
        for destination in destinations:
            cleaned_destination = destination.strip("<>")
            target = local_target(destination)
            if target and not (path.parent / target).exists():
                issues.append((line_number, cleaned_destination))
                continue

            parsed = urlsplit(destination.removeprefix("<").removesuffix(">"))
            if parsed.scheme or parsed.netloc or not parsed.fragment:
                continue
            target_path = path.parent / unquote(parsed.path) if parsed.path else path
            if target_path.suffix.lower() in MARKDOWN_SUFFIXES and target_path.exists():
                fragment = unquote(parsed.fragment)
                if fragment not in anchors(target_path):
                    issues.append((line_number, cleaned_destination))
    return issues


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path, default=[Path.cwd()])
    args = parser.parse_args()
    files = sorted({file for root in args.paths for file in markdown_files(root)})
    issue_count = 0
    for path in files:
        for line_number, destination in broken_links(path):
            print(f"{path}:{line_number}: local link does not exist: {destination}")
            issue_count += 1
    if issue_count:
        print(f"Markdown link lint failed with {issue_count} issue(s).")
        return 1
    print(f"Markdown link lint passed for {len(files)} file(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
