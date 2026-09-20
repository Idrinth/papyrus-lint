#!/usr/bin/env python3
"""Check that local links in Markdown files point to existing paths."""

from __future__ import annotations

import argparse
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit

MARKDOWN_SUFFIXES = {".md", ".markdown"}
SKIPPED_DIRECTORIES = {".git", "node_modules", "target"}
INLINE_LINK = re.compile(r"!?\[[^]]*\]\(\s*(<[^>]+>|[^\s)]+)")
REFERENCE_LINK = re.compile(r"^\s{0,3}\[[^]]+\]:\s*(<[^>]+>|\S+)")
FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
CODE_SPAN = re.compile(r"(`+)(.*?)\1")


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


def broken_links(path: Path) -> list[tuple[int, str]]:
    """Return line numbers and destinations for nonexistent local links."""
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
            target = local_target(destination)
            if target and not (path.parent / target).exists():
                issues.append((line_number, destination.strip("<>")))
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
