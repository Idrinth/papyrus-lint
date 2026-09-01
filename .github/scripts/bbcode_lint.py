#!/usr/bin/env python3
"""Check Nexus Mods BBCode for basic structural errors."""

from __future__ import annotations

import argparse
import re
from dataclasses import dataclass
from pathlib import Path


# Keep this deliberately conservative: tags outside this set are reported as
# typos rather than silently treated as plain text.
KNOWN_TAGS = {
    "*",
    "b",
    "center",
    "code",
    "color",
    "font",
    "heading",
    "i",
    "img",
    "left",
    "list",
    "quote",
    "right",
    "s",
    "size",
    "spoiler",
    "table",
    "td",
    "th",
    "tr",
    "u",
    "url",
}
TAG_RE = re.compile(r"\[(/?)([A-Za-z*][A-Za-z0-9]*)(?:=([^\]\r\n]*))?\]")


@dataclass(frozen=True)
class Issue:
    offset: int
    message: str


def lint(text: str) -> list[Issue]:
    """Return structural issues in *text*, in source order."""
    issues: list[Issue] = []
    stack: list[tuple[str, int]] = []

    for match in TAG_RE.finditer(text):
        closing, raw_name, argument = match.groups()
        name = raw_name.lower()
        if name not in KNOWN_TAGS:
            issues.append(Issue(match.start(), f"unknown tag [{raw_name}]"))
            continue

        if closing:
            if argument is not None:
                issues.append(Issue(match.start(), f"closing tag [/{raw_name}] cannot have an argument"))
            if not stack:
                issues.append(Issue(match.start(), f"closing tag [/{raw_name}] has no opening tag"))
            elif stack[-1][0] == name:
                stack.pop()
            else:
                expected = stack[-1][0]
                issues.append(
                    Issue(match.start(), f"closing tag [/{raw_name}] does not match open [{expected}]")
                )
            continue

        stack.append((name, match.start()))

    for name, offset in stack:
        issues.append(Issue(offset, f"tag [{name}] is not closed"))

    return sorted(issues, key=lambda issue: issue.offset)


def location(text: str, offset: int) -> tuple[int, int]:
    line = text.count("\n", 0, offset) + 1
    previous_newline = text.rfind("\n", 0, offset)
    return line, offset - previous_newline


def check_file(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    issues = lint(text)
    for issue in issues:
        line, column = location(text, issue.offset)
        print(f"{path}:{line}:{column}: {issue.message}")
    return len(issues)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+", type=Path)
    args = parser.parse_args()
    issue_count = sum(check_file(path) for path in args.files)
    if issue_count:
        print(f"BBCode lint failed with {issue_count} issue(s).")
        return 1
    print(f"BBCode lint passed for {len(args.files)} file(s).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
