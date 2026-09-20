"""Bake shared/links.yaml's contact links into the editor plugin READMEs.

Both plugin READMEs are packaged into their release artifacts as-is (the
VS Code `.vsix` and the SublimeLinter `.zip`), so this is run in the
release workflow's packaging jobs before vsce/zip run, mirroring how
cli_release_hashes.py rewrites those same plugins' checked-in files ahead
of packaging.
"""

from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path

from .links import render_markdown_list_items, replace_html_link_markers

REPO_ROOT = Path(__file__).resolve().parents[3]
README_PATHS = (
    REPO_ROOT / "vscode-extension" / "README.md",
    REPO_ROOT / "SublimeLinter-contrib-papyrus-lint" / "README.md",
)


def render_readme_links(text: str) -> str:
    """Fill a README's `<!--TAG-LINKS-->` / `<!--LINKS-->` markers."""
    return replace_html_link_markers(text, render_markdown_list_items)


def write_readme_links(paths: Sequence[Path] = README_PATHS) -> None:
    for path in paths:
        path.write_text(render_readme_links(path.read_text(encoding="utf-8")), encoding="utf-8")
