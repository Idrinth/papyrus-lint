"""CSS `@import` inlining for the Pages builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change.
"""

from __future__ import annotations

import re
from pathlib import Path

CSS_IMPORT_RE = re.compile(r"""@import\s+(?:url\(\s*["']?([^"')]+)["']?\s*\)|["']([^"']+)["'])\s*;""")


def inline_css_imports(text: str, origin: Path, seen: set[Path] | None = None) -> str:
    """Inlines relative `@import` rules so the deployed stylesheet stays a
    single file. Remote (`http:`, `https:`, protocol-relative) imports are
    left untouched. A missing or cyclical import fails the build rather
    than shipping a stylesheet that 404s a dependency at runtime."""
    origin = origin.resolve()
    visited = set() if seen is None else set(seen)
    if origin in visited:
        raise SystemExit(f"{origin}: cyclical @import")
    visited.add(origin)

    def replace(match: re.Match[str]) -> str:
        rel = match.group(1) or match.group(2)
        if rel.startswith(("http:", "https:", "//")):
            return match.group(0)
        imported = (origin.parent / rel).resolve()
        if not imported.is_file():
            raise SystemExit(f"{origin}: @import not found: {rel}")
        imported_text = imported.read_text(encoding="utf-8")
        inlined = inline_css_imports(imported_text, imported, visited)
        if inlined and not inlined.endswith("\n"):
            inlined += "\n"
        return inlined

    return CSS_IMPORT_RE.sub(replace, text)
