#!/usr/bin/env python3
"""Builds the GitHub Pages site from pages/index.template.html.

Substitutes the lint tables and CLI usage examples in the template with
content converted directly from README.md's own tables/code blocks, so
that documentation never has to be kept in sync by hand in two places.
Also renders every file listed in DOCS into its own browsable subpage
under docs/ (via pages/docs.template.html), and assembles the site's
assets/ directory by copying the screenshots and icon this page uses
from resources/ and app/src-tauri/icons, rather than committing
duplicate copies of them under pages/. Every generated HTML page and the
stylesheet are minified (see minify_html/minify_css) before being written
into the output directory.

Usage: pages/build.py [--out DIR]  (default DIR: pages/dist)
"""

from __future__ import annotations

import argparse
import html
import json
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent
DOCS_DIR = ROOT / "docs"

LINT_CATEGORIES = ["Formatting", "Performance", "Reliability", "Bugprone", "Other"]

GITHUB_BLOB_BASE = "https://github.com/idrinth/papyrus-lint/blob/the-one"
SITE_URL = "https://idrinth.github.io/papyrus-lint/"

# Every file in docs/ published as a browsable subpage, alongside a short
# hand-written blurb shown in the docs list on the homepage and on the docs
# index page. `kind` picks how build.py renders that file's own content.
DOCS = [
    {
        "filename": "github-actions-example.md",
        "slug": "github-actions-example",
        "kind": "markdown",
        "blurb": "A minimal GitHub Actions workflow that lints a project on every push and pull request.",
    },
    {
        "filename": "papyrus-lint.default.yaml",
        "slug": "papyrus-lint-default-yaml",
        "kind": "yaml",
        "title": "Default configuration (papyrus-lint.yaml)",
        "description": (
            "The full papyrus-lint.yaml written into a project with no configuration file yet, with every key's "
            "default value documented inline. See the README's configuration reference for what each key does."
        ),
        "blurb": "The full default papyrus-lint.yaml, with every key's default value documented inline.",
    },
    {
        "filename": "papyrus-lint-report.schema.json",
        "slug": "papyrus-lint-report-schema",
        "kind": "json-schema",
        "blurb": "The JSON Schema for the report PapyrusLinterCLI --json emits.",
    },
    {
        "filename": "ast-cache-entry.schema.json",
        "slug": "ast-cache-entry-schema",
        "kind": "json-schema",
        "blurb": "The JSON Schema for one entry in the on-disk ast-cache used to skip re-parsing unchanged scripts.",
    },
    {
        "filename": "nexuspage.bbcode",
        "slug": "nexuspage-bbcode",
        "kind": "bbcode",
        "title": "Nexus Mods page description (BBCode source)",
        "description": "The BBCode source used for the project's listing on Nexus Mods, kept in sync with the README by hand.",
        "blurb": "The BBCode source behind the project's Nexus Mods page listing.",
    },
]

DOC_FILENAME_TO_SLUG = {doc["filename"]: doc["slug"] for doc in DOCS}

# Simple list of YouTube video IDs/titles rendered onto videos.html, so a new
# video can be added without touching build.py or its template.
VIDEOS_FILE = PAGES_DIR / "videos.json"

ASSETS = {
    "logo-small.jpg": ROOT / "resources" / "logo-small.jpg",
    "logo.jpg": ROOT / "resources" / "logo.jpg",
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

PRE_BLOCK_RE = re.compile(r"<pre\b[^>]*>.*?</pre>", re.DOTALL | re.IGNORECASE)
HTML_COMMENT_RE = re.compile(r"<!--.*?-->", re.DOTALL)
TAG_GAP_RE = re.compile(r">\s+<")
LINE_INDENT_RE = re.compile(r"[ \t]*\n[ \t]*")
BLANK_LINES_RE = re.compile(r"\n{2,}")
CSS_COMMENT_RE = re.compile(r"/\*.*?\*/", re.DOTALL)
CSS_WHITESPACE_RUN_RE = re.compile(r"\s+")
CSS_SYNTAX_SPACE_RE = re.compile(r"\s*([{}:;,])\s*")
CSS_TRAILING_SEMICOLON_RE = re.compile(r";}")


def minify_html(text: str) -> str:
    """Minifies static HTML for deployment: strips comments and collapses
    insignificant indentation/whitespace, while leaving <pre>...</pre>
    blocks untouched since their whitespace (the CLI/config/schema
    examples) is significant."""
    blocks: list[str] = []

    def stash(match: re.Match[str]) -> str:
        blocks.append(match.group(0))
        return f"\x00{len(blocks) - 1}\x00"

    result = PRE_BLOCK_RE.sub(stash, text)
    result = HTML_COMMENT_RE.sub("", result)
    result = TAG_GAP_RE.sub("><", result)
    result = LINE_INDENT_RE.sub("\n", result)
    result = BLANK_LINES_RE.sub("\n", result)
    result = result.strip()
    return re.sub(r"\x00(\d+)\x00", lambda m: blocks[int(m.group(1))], result)


def minify_css(text: str) -> str:
    """Minifies CSS for deployment: strips comments and collapses
    whitespace, which carries no meaning in this stylesheet's syntax
    outside of string/url literals (none of which contain whitespace
    here)."""
    result = CSS_COMMENT_RE.sub("", text)
    result = CSS_WHITESPACE_RUN_RE.sub(" ", result).strip()
    result = CSS_SYNTAX_SPACE_RE.sub(r"\1", result)
    return CSS_TRAILING_SEMICOLON_RE.sub("}", result)


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


def render_inline(text: str, link_rewrite=None) -> str:
    """Converts a small subset of inline Markdown (links, code spans, bold)
    used in README.md's/docs/*.md's tables/prose into HTML, escaping
    everything else. `link_rewrite`, when given, maps a link's raw href
    (e.g. a repo-relative path) to the href that should actually be emitted."""
    escaped = html.escape(text, quote=False)

    def link(m: re.Match[str]) -> str:
        href = m.group(2)
        if link_rewrite is not None:
            href = link_rewrite(href)
        href = href.replace('"', "&quot;")
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


def resolve_doc_href(href: str) -> str:
    """Rewrites a link target found inside a docs/*.md file so it works from
    a published subpage: a link to another published doc resolves to that
    doc's own subpage, a link into the repository resolves on GitHub."""
    if href in DOC_FILENAME_TO_SLUG:
        return f"{DOC_FILENAME_TO_SLUG[href]}.html"
    if href.startswith("../"):
        return f"{GITHUB_BLOB_BASE}/{href[len('../'):]}"
    return href


def strip_markdown_inline(text: str) -> str:
    """Reduces a small subset of inline Markdown to plain text, for use
    where HTML markup isn't allowed (an HTML attribute value)."""
    text = INLINE_LINK_RE.sub(r"\1", text)
    return text.replace("`", "").replace("**", "")


def first_paragraph(lines: list[str]) -> str:
    """Returns the first non-blank, non-heading paragraph in a Markdown
    document's lines, its own line breaks collapsed into spaces."""
    para: list[str] = []
    for line in lines:
        stripped = line.strip()
        if not stripped or HEADING_RE.match(line) or stripped.startswith("```"):
            if para:
                break
            continue
        para.append(stripped)
    return " ".join(para)


def markdown_to_html(lines: list[str], link_rewrite=None) -> str:
    """Converts the small subset of Markdown used by docs/*.md (headings,
    paragraphs, fenced code blocks, and render_inline's inline formatting)
    into HTML."""
    out: list[str] = []
    para: list[str] = []

    def flush_paragraph() -> None:
        if para:
            out.append(f"<p>{render_inline(' '.join(para), link_rewrite)}</p>")
            para.clear()

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()
        if stripped.startswith("```"):
            flush_paragraph()
            i += 1
            code_lines: list[str] = []
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            i += 1
            out.append(f'<pre class="code-block"><code>{html.escape(chr(10).join(code_lines))}</code></pre>')
            continue
        heading = HEADING_RE.match(line)
        if heading:
            flush_paragraph()
            level = len(heading.group(1))
            out.append(f"<h{level}>{render_inline(heading.group(2), link_rewrite)}</h{level}>")
            i += 1
            continue
        if not stripped:
            flush_paragraph()
            i += 1
            continue
        para.append(stripped)
        i += 1
    flush_paragraph()
    return "\n".join(out)


def raw_github_link(doc: dict) -> str:
    href = f"{GITHUB_BLOB_BASE}/docs/{doc['filename']}"
    return f'<p><a class="doc-raw-link" href="{href}">View raw source on GitHub &rarr;</a></p>'


def render_doc(doc: dict) -> tuple[str, str, str]:
    """Renders one docs/ file into (title, plain-text description, content
    HTML) for its published subpage."""
    source = (DOCS_DIR / doc["filename"]).read_text(encoding="utf-8")
    kind = doc["kind"]
    if kind == "markdown":
        lines = source.splitlines()
        title_match = HEADING_RE.match(lines[0]) if lines else None
        if title_match and len(title_match.group(1)) == 1:
            title = title_match.group(2)
            body_lines = lines[1:]
        else:
            title = doc["filename"]
            body_lines = lines
        description = strip_markdown_inline(first_paragraph(body_lines))
        content_html = markdown_to_html(body_lines, resolve_doc_href)
    elif kind == "json-schema":
        data = json.loads(source)
        title = data.get("title", doc["filename"])
        description = data.get("description", "")
        content_html = f'<pre class="code-block"><code>{html.escape(json.dumps(data, indent=2))}</code></pre>'
    else:
        title = doc["title"]
        description = doc["description"]
        content_html = f'<pre class="code-block"><code>{html.escape(source)}</code></pre>'
    content_html += raw_github_link(doc)
    return title, description, content_html


def render_docs_list_items(doc_results: dict, link_prefix: str) -> str:
    items = []
    for doc in DOCS:
        info = doc_results[doc["slug"]]
        items.append(
            "<li>"
            f'<a href="{link_prefix}{doc["slug"]}.html">{html.escape(info["title"])}</a>'
            f'<p>{html.escape(doc["blurb"])}</p>'
            "</li>"
        )
    return "\n".join(items)


def build_doc_pages(out_dir: Path, doc_results: dict) -> None:
    docs_out_dir = out_dir / "docs"
    docs_out_dir.mkdir()
    docs_template = (PAGES_DIR / "docs.template.html").read_text(encoding="utf-8")

    def render_page(title: str, description: str, content_html: str, url: str) -> str:
        page = docs_template.replace("<!--DOC_TITLE-->", html.escape(title))
        page = page.replace("<!--DOC_DESCRIPTION-->", html.escape(description, quote=True))
        page = page.replace("<!--DOC_URL-->", html.escape(url, quote=True))
        return page.replace("<!--DOC_CONTENT-->", content_html)

    for doc in DOCS:
        info = doc_results[doc["slug"]]
        page = render_page(
            info["title"], info["description"], info["content_html"], f"{SITE_URL}docs/{doc['slug']}.html"
        )
        (docs_out_dir / f"{doc['slug']}.html").write_text(minify_html(page), encoding="utf-8")

    index_content = f'<ul class="docs-list">{render_docs_list_items(doc_results, "")}</ul>'
    index_page = render_page(
        "Documentation",
        "Reference material from the project's docs/ directory, published as browsable pages.",
        index_content,
        f"{SITE_URL}docs/index.html",
    )
    (docs_out_dir / "index.html").write_text(minify_html(index_page), encoding="utf-8")


def render_videos_list(videos: list[dict]) -> str:
    items = []
    for video in videos:
        video_id = html.escape(video["id"], quote=True)
        title = html.escape(video["title"])
        items.append(
            '<figure class="video-card">'
            '<div class="video-card__frame">'
            f'<iframe src="https://www.youtube-nocookie.com/embed/{video_id}" title="{title}" '
            'loading="lazy" allow="encrypted-media; picture-in-picture" allowfullscreen></iframe>'
            "</div>"
            f"<figcaption>{title}</figcaption>"
            "</figure>"
        )
    return "\n".join(items)


def build_videos_page(out_dir: Path) -> None:
    videos = json.loads(VIDEOS_FILE.read_text(encoding="utf-8"))
    template = (PAGES_DIR / "videos.template.html").read_text(encoding="utf-8")
    if "<!--VIDEOS_LIST-->" not in template:
        raise SystemExit("videos.template.html: missing marker <!--VIDEOS_LIST-->")
    page = template.replace("<!--VIDEOS_LIST-->", render_videos_list(videos))
    (out_dir / "videos.html").write_text(minify_html(page), encoding="utf-8")


def build(out_dir: Path, version: str = "") -> None:
    readme_lines = (ROOT / "README.md").read_text(encoding="utf-8").splitlines()
    lints_section = extract_section(readme_lines, "Implemented Lints", level=2)
    cli_section = extract_section(readme_lines, "Command-line interface", level=2)

    lint_tables = {
        category: render_lint_table(extract_section(lints_section, category, level=3))
        for category in LINT_CATEGORIES
    }
    cli_examples = html.escape(first_code_block(cli_section))

    doc_results = {}
    for doc in DOCS:
        title, description, content_html = render_doc(doc)
        doc_results[doc["slug"]] = {"title": title, "description": description, "content_html": content_html}

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
    if "<!--DOCS_LIST-->" not in template:
        raise SystemExit("index.template.html: missing marker <!--DOCS_LIST-->")
    template = template.replace("<!--DOCS_LIST-->", render_docs_list_items(doc_results, "docs/"))
    template = template.replace("<!--VERSION-->", html.escape(version) if version else "unreleased")

    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    (out_dir / "index.html").write_text(minify_html(template), encoding="utf-8")
    css = (PAGES_DIR / "styles.css").read_text(encoding="utf-8")
    (out_dir / "styles.css").write_text(minify_css(css), encoding="utf-8")

    assets_dir = out_dir / "assets"
    assets_dir.mkdir()
    for name, source in ASSETS.items():
        shutil.copyfile(source, assets_dir / name)

    shutil.copytree(PAGES_DIR / "fonts", out_dir / "fonts")

    build_doc_pages(out_dir, doc_results)
    build_videos_page(out_dir)


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
