#!/usr/bin/env python3
"""Builds the GitHub Pages site from pages/index.template.html.

Substitutes the lint tables and CLI usage examples in the template with
content converted directly from README.md's own tables/code blocks, so
that documentation never has to be kept in sync by hand in two places.
Also assembles the site's assets/ directory by copying the screenshots
and icon this page uses from resources/ and app/src-tauri/icons, rather
than committing duplicate copies of them under pages/.

Usage: pages/build.py [--out DIR]  (default DIR: pages/dist)
"""

from __future__ import annotations

import argparse
import html
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent

LINT_CATEGORIES = ["Formatting", "Performance", "Reliability", "Bugprone", "Other"]

ASSETS = {
    "logo-small.jpg": ROOT / "resources" / "logo-small.jpg",
    "papyrus-lint-import.png": ROOT / "resources" / "papyrus-lint-import.png",
    "papyrus-lint-results.png": ROOT / "resources" / "papyrus-lint-results.png",
    "papyrus-lint-viewer.png": ROOT / "resources" / "papyrus-lint-viewer.png",
    "papyrus-lint-vscode.png": ROOT / "resources" / "papyrus-lint-vscode.png",
    "papyrus-lint-cli.png": ROOT / "resources" / "papyrus-lint-cli.png",
    "favicon.png": ROOT / "app" / "src-tauri" / "icons" / "icon.png",
}

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
INLINE_LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
INLINE_CODE_RE = re.compile(r"`([^`]+)`")
INLINE_BOLD_RE = re.compile(r"\*\*([^*]+)\*\*")
ROW_SPLIT_RE = re.compile(r"(?<!\\)\|")


def extract_section(lines: list[str], heading_text: str, level: int) -> list[str]:
    """Returns the lines strictly between a heading and the next heading at
    the same level or shallower."""
    start = None
    for i, line in enumerate(lines):
        m = HEADING_RE.match(line)
        if m and len(m.group(1)) == level and m.group(2) == heading_text:
            start = i + 1
            break
    if start is None:
        raise SystemExit(f"README.md: heading not found: {'#' * level} {heading_text}")
    end = len(lines)
    for i in range(start, len(lines)):
        m = HEADING_RE.match(lines[i])
        if m and len(m.group(1)) <= level:
            end = i
            break
    return lines[start:end]


def render_inline(text: str) -> str:
    """Converts a small subset of inline Markdown (links, code spans, bold)
    used in README.md's tables/prose into HTML, escaping everything else."""
    escaped = html.escape(text, quote=False)

    def link(m: re.Match[str]) -> str:
        href = m.group(2).replace('"', "&quot;")
        return f'<a href="{href}">{m.group(1)}</a>'

    escaped = INLINE_LINK_RE.sub(link, escaped)
    escaped = INLINE_CODE_RE.sub(r"<code>\1</code>", escaped)
    escaped = INLINE_BOLD_RE.sub(r"<strong>\1</strong>", escaped)
    return escaped


def split_table_row(line: str) -> list[str]:
    line = line.strip()
    if line.startswith("|"):
        line = line[1:]
    if line.endswith("|"):
        line = line[:-1]
    return [cell.replace("\\|", "|").strip() for cell in ROW_SPLIT_RE.split(line)]


def render_lint_table(section_lines: list[str]) -> str:
    rows = [line for line in section_lines if line.strip().startswith("|")]
    if len(rows) < 3:
        raise SystemExit("README.md: expected a Lint/Description/Auto-Fix table, found none")
    header = split_table_row(rows[0])
    # rows[1] is the "| --- | --- | --- |" separator row.
    out = ['<div class="lint-table-wrap">', '<table class="lint-table">', "<thead><tr>"]
    for cell in header:
        out.append(f"<th>{html.escape(cell)}</th>")
    out.append("</tr></thead>")
    out.append("<tbody>")
    for raw_row in rows[2:]:
        cells = split_table_row(raw_row)
        name, desc = cells[0], cells[1]
        fix = cells[2] if len(cells) > 2 else ""
        out.append("<tr>")
        out.append(f"<td>{render_inline(name)}</td>")
        out.append(f"<td>{render_inline(desc)}</td>")
        out.append(f'<td class="fix-yes">✓</td>' if fix.strip() else "<td></td>")
        out.append("</tr>")
    out.append("</tbody></table></div>")
    return "\n".join(out)


def first_code_block(section_lines: list[str]) -> str:
    start = end = None
    for i, line in enumerate(section_lines):
        if line.strip().startswith("```"):
            start = i
            break
    if start is None:
        raise SystemExit("README.md: expected a fenced code block, found none")
    for i in range(start + 1, len(section_lines)):
        if section_lines[i].strip().startswith("```"):
            end = i
            break
    if end is None:
        raise SystemExit("README.md: unterminated fenced code block")
    return "\n".join(section_lines[start + 1 : end])


def build(out_dir: Path, version: str = "") -> None:
    readme_lines = (ROOT / "README.md").read_text(encoding="utf-8").splitlines()
    lints_section = extract_section(readme_lines, "Implemented Lints", level=2)
    cli_section = extract_section(readme_lines, "Command-line interface", level=2)

    lint_tables = {
        category: render_lint_table(extract_section(lints_section, category, level=3))
        for category in LINT_CATEGORIES
    }
    cli_examples = html.escape(first_code_block(cli_section))

    template = (PAGES_DIR / "index.template.html").read_text(encoding="utf-8")
    for category, table_html in lint_tables.items():
        marker = f"<!--LINT_TABLE:{category}-->"
        if marker not in template:
            raise SystemExit(f"index.template.html: missing marker {marker}")
        template = template.replace(marker, table_html)
    template = template.replace(
        "<!--CLI_EXAMPLES-->",
        f'<pre class="code-block cli-examples"><code>{cli_examples}</code></pre>',
    )
    template = template.replace("<!--VERSION-->", html.escape(version) if version else "unreleased")

    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    (out_dir / "index.html").write_text(template, encoding="utf-8")
    shutil.copyfile(PAGES_DIR / "styles.css", out_dir / "styles.css")

    assets_dir = out_dir / "assets"
    assets_dir.mkdir()
    for name, source in ASSETS.items():
        shutil.copyfile(source, assets_dir / name)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=PAGES_DIR / "dist")
    parser.add_argument(
        "--version",
        default="",
        help="Version tag to display on the site (e.g. v1.2.3); shown as 'unreleased' if omitted",
    )
    args = parser.parse_args()
    build(args.out, args.version)
    print(f"Built site into {args.out}")


if __name__ == "__main__":
    main()
