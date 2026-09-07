#!/usr/bin/env python3
"""Builds the GitHub Pages site from pages/index.template.html.

Substitutes the lint tables and CLI usage examples in the template with
content converted directly from README.md's own tables/code blocks, so
that documentation never has to be kept in sync by hand in two places.
Also renders every file listed in DOCS into its own browsable subpage
under docs/ (via pages/docs.template.html), and assembles the site's
assets/ directory by copying the screenshots and icon this page uses
from resources/ and app/src-tauri/icons, rather than committing
duplicate copies of them under pages/. Every asset actually rendered as
an <img> also gets a WebP and an AVIF sibling (see
convert_to_modern_formats), and every such <img> tag, in every generated
page, is rewritten into a <picture> offering those smaller formats ahead
of the original as a fallback (see wrap_images_with_modern_sources).
Every generated HTML page and the stylesheet are minified (see
minify_html/minify_css) before being written into the output directory.
Also renders coverage.html (via pages/coverage.template.html), a per-module,
per-file line coverage breakdown built from a directory of downloaded lcov
reports passed as --coverage-dir (see build_coverage_content), reusing
.github/scripts/coverage_summary.py's module grouping so it can't drift
from the coverage figures already shown in release notes and PR comments;
omitting --coverage-dir (e.g. a local preview build) renders the page with
a "data unavailable" placeholder instead of failing the build. Also writes
a sitemap.xml (every page build() renders, see sitemap_urls) and a
robots.txt pointing at it, and copies pages/CNAME into the output
directory so GitHub Pages keeps serving the site's custom domain across
each Actions-based deploy.

Usage: pages/build.py [--out DIR] [--version TAG] [--coverage-dir DIR]  (default DIR: pages/dist)
"""

from __future__ import annotations

import argparse
import html
import importlib.util
import json
import re
import shutil
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent
DOCS_DIR = ROOT / "docs"

# .github/scripts isn't an importable package (its directory name starts
# with a dot), so the coverage subpage loads it by file path instead. This
# reuses coverage_summary.py's MODULES grouping and lcov parsing rather than
# duplicating them, so the subpage can never drift from the module
# breakdown used in the release notes and PR coverage comments.
COVERAGE_SUMMARY_SCRIPT = ROOT / ".github" / "scripts" / "coverage_summary.py"

# Matches a CI runner's absolute checkout prefix in an lcov SF: path (e.g.
# /home/runner/work/papyrus-lint/papyrus-lint/app/...), stripped off so the
# coverage subpage shows paths relative to the repository root.
REPO_CHECKOUT_MARKER = "/papyrus-lint/"

LINT_CATEGORIES = ["Formatting", "Performance", "Reliability", "Bugprone", "Other"]

GITHUB_BLOB_BASE = "https://github.com/idrinth/papyrus-lint/blob/the-one"
SITE_URL = "https://papyrus-lint.idrinth.de/"
CNAME_FILE = PAGES_DIR / "CNAME"

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
        "filename": "papyrus-lint-action-readme.md",
        "slug": "papyrus-lint-action-readme",
        "kind": "markdown",
        "source_url": "https://github.com/Idrinth/papyrus-lint-action/blob/the-one/README.md",
        "blurb": (
            "The papyrus-lint-action GitHub Action's own README: its inputs, outputs, and how it "
            "posts findings as pull request review comments."
        ),
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
        "description": (
            "The JSON Schema for one entry in the on-disk ast-cache used to skip re-parsing unchanged scripts, "
            "keyed by the cached script's content hash, modification time, and the linter version that wrote it."
        ),
        "blurb": "The JSON Schema for one entry in the on-disk ast-cache used to skip re-parsing unchanged scripts.",
    },
    {
        "filename": "nexuspage.bbcode",
        "slug": "nexuspage-bbcode",
        "kind": "bbcode",
        "title": "Nexus Mods page description (BBCode source)",
        "description": (
            "The BBCode source used for the project's listing on Nexus Mods, kept in sync with the README by hand."
        ),
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

# The ASSETS entries actually rendered as <img> elements on the page (as
# opposed to logo.jpg, only ever referenced as a raw og:image/twitter:image
# URL, and favicon.png, only ever referenced via <link rel="icon">): these
# get a WebP and an AVIF sibling generated alongside the original, so
# wrap_images_with_modern_sources can offer them as smaller <picture>
# alternatives.
MODERN_FORMAT_ASSETS = {
    "logo-small.jpg",
    "papyrus-lint-import.png",
    "papyrus-lint-results.png",
    "papyrus-lint-viewer.png",
    "papyrus-lint-vscode.png",
    "papyrus-lint-cli.png",
}

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
INLINE_LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
INLINE_CODE_RE = re.compile(r"`([^`]+)`")
INLINE_BOLD_RE = re.compile(r"\*\*([^*]+)\*\*")
ROW_SPLIT_RE = re.compile(r"(?<!\\)\|")

PRE_BLOCK_RE = re.compile(r"<pre\b[^>]*>.*?</pre>", re.DOTALL | re.IGNORECASE)
HTML_COMMENT_RE = re.compile(r"<!--.*?-->", re.DOTALL)
TAG_GAP_RE = re.compile(r">\s+<")
WHITESPACE_RUN_RE = re.compile(r"\s+")
CSS_COMMENT_RE = re.compile(r"/\*.*?\*/", re.DOTALL)
CSS_WHITESPACE_RUN_RE = re.compile(r"\s+")
CSS_SYNTAX_SPACE_RE = re.compile(r"\s*([{}:;,])\s*")
CSS_TRAILING_SEMICOLON_RE = re.compile(r";}")

# Matches a local (not http(s), e.g. a shields.io badge) <img> tag whose src
# points into assets/ (optionally prefixed with ../, as docs/*.html pages do).
IMG_ASSET_TAG_RE = re.compile(r'<img\b[^>]*\ssrc="((?:\.\./)?assets/([\w.-]+)\.(?:png|jpg))"[^>]*/?>')


def minify_html(text: str) -> str:
    """Minifies static HTML for deployment: strips comments and collapses
    every run of insignificant whitespace (including line breaks) down to
    a single space, while leaving <pre>...</pre> blocks untouched since
    their whitespace (the CLI/config/schema examples) is significant."""
    blocks: list[str] = []

    def stash(match: re.Match[str]) -> str:
        blocks.append(match.group(0))
        return f"\x00{len(blocks) - 1}\x00"

    result = PRE_BLOCK_RE.sub(stash, text)
    result = HTML_COMMENT_RE.sub("", result)
    result = TAG_GAP_RE.sub("><", result)
    result = WHITESPACE_RUN_RE.sub(" ", result)
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
        href = html.unescape(m.group(2))
        if link_rewrite is not None:
            href = link_rewrite(href)
        href = html.escape(href, quote=True)
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
        out.append('<td class="fix-yes">✓</td>' if fix.strip() else "<td></td>")
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


def convert_to_modern_formats(source: Path, dest_dir: Path) -> None:
    """Writes a WebP and an AVIF sibling of an already-copied asset (a PNG
    screenshot or the header's JPEG logo) into dest_dir, so
    wrap_images_with_modern_sources can offer them as smaller <picture>
    alternatives to the original format. PNGs (the screenshots) are
    re-encoded losslessly, since they're UI screenshots where lossy
    artifacts around text/lines would be conspicuous; the JPEG logo is
    re-encoded lossy, matching its own already-lossy source format."""
    lossless = source.suffix.lower() == ".png"
    with Image.open(source) as image:
        if not lossless and image.mode != "RGB":
            image = image.convert("RGB")
        image.save(dest_dir / f"{source.stem}.webp", lossless=lossless, quality=80)
        image.save(dest_dir / f"{source.stem}.avif", lossless=lossless, quality=65)


def wrap_images_with_modern_sources(page_html: str) -> str:
    """Wraps every <img> tag whose src points at a MODERN_FORMAT_ASSETS
    asset in a <picture> element offering the AVIF/WebP siblings
    convert_to_modern_formats generates as preferred <source>s, keeping the
    original <img> as the final (and oldest-browser-compatible) fallback."""

    def wrap(match: re.Match[str]) -> str:
        src, name = match.group(1), match.group(2)
        if f"{name}{Path(src).suffix}" not in MODERN_FORMAT_ASSETS:
            return match.group(0)
        prefix = src[: -len(Path(src).name)]
        return (
            "<picture>"
            f'<source srcset="{prefix}{name}.avif" type="image/avif" />'
            f'<source srcset="{prefix}{name}.webp" type="image/webp" />'
            f"{match.group(0)}"
            "</picture>"
        )

    return IMG_ASSET_TAG_RE.sub(wrap, page_html)


def finalize_page(page_html: str) -> str:
    """Applies every page-wide HTML post-processing step - modern-format
    <picture> wrapping, then minification - shared by every full page this
    builder writes out."""
    return minify_html(wrap_images_with_modern_sources(page_html))


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
    in_code_block = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("```"):
            in_code_block = not in_code_block
            if para:
                break
            continue
        if in_code_block:
            continue
        if not stripped or HEADING_RE.match(line):
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
            code_html = html.escape(chr(10).join(code_lines))
            out.append(f'<pre class="code-block" tabindex="0"><code>{code_html}</code></pre>')
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
    """Links back to the doc's own raw source on GitHub: a checked-in
    docs/ file by default, or `source_url` when a doc's content is a copy
    of a file from another repository (e.g. papyrus-lint-action's README)."""
    href = doc.get("source_url", f"{GITHUB_BLOB_BASE}/docs/{doc['filename']}")
    return (
        f'<p><a class="doc-raw-link" href="{html.escape(href, quote=True)}">'
        "View raw source on GitHub &rarr;</a></p>"
    )


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
        # A schema's own "description" is written for JSON Schema consumers and can run
        # much longer than a page tagline should be (see ast-cache-entry.schema.json,
        # whose 800+ character description became this page's Largest Contentful Paint
        # element); prefer a DOCS entry's own short "description" when it sets one.
        description = doc.get("description", data.get("description", ""))
        schema_html = html.escape(json.dumps(data, indent=2))
        content_html = f'<pre class="code-block" tabindex="0"><code>{schema_html}</code></pre>'
    else:
        title = doc["title"]
        description = doc["description"]
        source_html = html.escape(source)
        content_html = f'<pre class="code-block" tabindex="0"><code>{source_html}</code></pre>'
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
        (docs_out_dir / f"{doc['slug']}.html").write_text(finalize_page(page), encoding="utf-8")

    index_content = f'<ul class="docs-list">{render_docs_list_items(doc_results, "")}</ul>'
    index_page = render_page(
        "Documentation",
        "Reference material from the project's docs/ directory, published as browsable pages.",
        index_content,
        f"{SITE_URL}docs/index.html",
    )
    (docs_out_dir / "index.html").write_text(finalize_page(index_page), encoding="utf-8")


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
    (out_dir / "videos.html").write_text(finalize_page(page), encoding="utf-8")


def load_coverage_summary():
    """Loads .github/scripts/coverage_summary.py by file path (see
    COVERAGE_SUMMARY_SCRIPT above) so the coverage subpage shares its
    MODULES grouping and pct() formatting instead of duplicating them."""
    spec = importlib.util.spec_from_file_location("coverage_summary", COVERAGE_SUMMARY_SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def normalize_source_path(raw_path: str) -> str:
    """Strips a CI runner's absolute checkout prefix off an lcov SF: path, so
    the coverage subpage displays paths relative to the repository root the
    same way the rest of the site links into it. A path that's already
    relative (as some coverage tools emit) is returned unchanged."""
    normalized = raw_path.strip().replace("\\", "/")
    marker_at = normalized.rfind(REPO_CHECKOUT_MARKER)
    if marker_at == -1:
        return normalized
    return normalized[marker_at + len(REPO_CHECKOUT_MARKER) :]


def parse_lcov_files(path: Path) -> list[tuple[str, int, int]] | None:
    """Returns (source_file, lines_found, lines_hit) for every SF:/
    end_of_record record in an lcov.info file, or None if the file doesn't
    exist. The per-file counterpart of coverage_summary.parse_lcov, which
    only sums the whole report."""
    if not path.is_file():
        return None
    records: list[tuple[str, int, int]] = []
    current_file: str | None = None
    found = hit = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith("SF:"):
            current_file = normalize_source_path(line[3:])
            found = hit = 0
        elif line.startswith("LF:"):
            found += int(line[3:])
        elif line.startswith("LH:"):
            hit += int(line[3:])
        elif line.startswith("end_of_record"):
            if current_file is not None:
                records.append((current_file, found, hit))
            current_file = None
    return records


def render_coverage_table(rows: list[tuple[str, int, int]], coverage_summary) -> str:
    if not rows:
        return '<p class="section-intro">No files reported.</p>'
    out = [
        '<div class="lint-table-wrap">',
        '<table class="lint-table">',
        "<thead><tr><th>File</th><th>Coverage</th><th>Lines</th></tr></thead>",
        "<tbody>",
    ]
    for name, found, hit in rows:
        out.append("<tr>")
        out.append(f"<td><code>{html.escape(name)}</code></td>")
        out.append(f"<td>{coverage_summary.pct(hit, found)}</td>")
        out.append(f"<td>{hit}/{found}</td>")
        out.append("</tr>")
    out.append("</tbody></table></div>")
    return "\n".join(out)


def render_coverage_entry(coverage_dir: Path, name: str, value, coverage_summary) -> tuple[int, int, bool, str]:
    """Renders one (name, value) MODULES entry's body (its own heading not
    included, since the caller decides whether to show it), recursing into
    nested groups the same way coverage_summary.render_entry does, since a
    value can be either a leaf lcov.info path or a further nested list of
    (label, value) entries (e.g. App's Crates). Returns (lines_found,
    lines_hit, whether any report was found, rendered body HTML)."""
    if isinstance(value, str):
        rows = parse_lcov_files(coverage_dir / value)
        if rows is None:
            return 0, 0, False, '<p class="section-intro">No report.</p>'
        found = sum(f for _, f, _h in rows)
        hit = sum(h for _, _f, h in rows)
        rows_sorted = sorted(rows, key=lambda r: ((r[2] / r[1]) if r[1] else 1.0, r[0]))
        return found, hit, True, render_coverage_table(rows_sorted, coverage_summary)

    found = hit = 0
    any_report = False
    child_html: list[str] = []
    for child_name, child_value in value:
        child_found, child_hit, child_any_report, child_body = render_coverage_entry(
            coverage_dir, child_name, child_value, coverage_summary
        )
        found += child_found
        hit += child_hit
        any_report = any_report or child_any_report
        heading = (
            f"<h3>{html.escape(child_name)} — {coverage_summary.pct(child_hit, child_found)} "
            f"({child_hit}/{child_found})</h3>"
        )
        child_html.append((heading if len(value) > 1 else "") + child_body)

    return found, hit, any_report, "".join(child_html)


def build_coverage_content(coverage_dir: Path, coverage_summary) -> str:
    """Renders the coverage subpage's body: a per-module, per-file line
    coverage breakdown from the downloaded lcov reports, worst-covered file
    first within each part so weak spots are immediately visible."""
    out: list[str] = []
    total_found = total_hit = 0
    any_report = False

    for label, parts in coverage_summary.MODULES:
        module_found = module_hit = 0
        part_html: list[str] = []
        for name, value in parts:
            found, hit, part_any_report, body = render_coverage_entry(coverage_dir, name, value, coverage_summary)
            module_found += found
            module_hit += hit
            total_found += found
            total_hit += hit
            any_report = any_report or part_any_report
            heading = (
                f"<h3>{html.escape(name)} — {coverage_summary.pct(hit, found)} ({hit}/{found})</h3>"
                if len(parts) > 1
                else ""
            )
            part_html.append(heading + body)

        summary = coverage_summary.pct(module_hit, module_found)
        out.append(
            f'<div class="coverage-module"><h2>{html.escape(label)} — {summary} '
            f"({module_hit}/{module_found})</h2>" + "".join(part_html) + "</div>"
        )

    total_summary = coverage_summary.pct(total_hit, total_found) if any_report else "n/a"
    out.insert(
        0,
        f'<p class="section-intro">Total line coverage: <strong>{total_summary}</strong> '
        f"({total_hit}/{total_found}).</p>",
    )
    return "\n".join(out)


def build_coverage_page(out_dir: Path, coverage_dir: Path | None, version: str) -> None:
    template = (PAGES_DIR / "coverage.template.html").read_text(encoding="utf-8")
    if "<!--COVERAGE_CONTENT-->" not in template:
        raise SystemExit("coverage.template.html: missing marker <!--COVERAGE_CONTENT-->")
    if coverage_dir is not None and coverage_dir.is_dir():
        content = build_coverage_content(coverage_dir, load_coverage_summary())
    else:
        content = '<p class="section-intro">Coverage data isn\'t available for this build.</p>'
    page = template.replace("<!--COVERAGE_CONTENT-->", content)
    page = page.replace("<!--COVERAGE_VERSION-->", html.escape(version) if version else "unreleased")
    (out_dir / "coverage.html").write_text(finalize_page(page), encoding="utf-8")


def sitemap_urls(doc_results: dict) -> list[str]:
    """Every page build() renders, as absolute SITE_URL-rooted URLs, in the
    same order sitemap.xml lists them. Kept in one place so the sitemap can
    never drift from the pages actually published."""
    urls = [SITE_URL, f"{SITE_URL}videos.html", f"{SITE_URL}coverage.html", f"{SITE_URL}docs/index.html"]
    for doc in DOCS:
        if doc["slug"] in doc_results:
            urls.append(f"{SITE_URL}docs/{doc['slug']}.html")
    return urls


def build_sitemap(out_dir: Path, doc_results: dict) -> None:
    entries = "\n".join(
        f"  <url><loc>{html.escape(url, quote=True)}</loc></url>" for url in sitemap_urls(doc_results)
    )
    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        f"{entries}\n"
        "</urlset>\n"
    )
    (out_dir / "sitemap.xml").write_text(xml, encoding="utf-8")


def build_robots_txt(out_dir: Path) -> None:
    content = f"User-agent: *\nAllow: /\n\nSitemap: {SITE_URL}sitemap.xml\n"
    (out_dir / "robots.txt").write_text(content, encoding="utf-8")


def build(out_dir: Path, version: str = "", coverage_dir: Path | None = None) -> None:
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
    cli_marker = "<!--CLI_EXAMPLES-->"
    if cli_marker not in template:
        raise SystemExit(f"index.template.html: missing marker {cli_marker}")
    template = template.replace(
        cli_marker, f'<pre class="code-block cli-examples"><code>{cli_examples}</code></pre>'
    )
    if "<!--DOCS_LIST-->" not in template:
        raise SystemExit("index.template.html: missing marker <!--DOCS_LIST-->")
    template = template.replace("<!--DOCS_LIST-->", render_docs_list_items(doc_results, "docs/"))
    template = template.replace("<!--VERSION-->", html.escape(version) if version else "unreleased")

    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    (out_dir / "index.html").write_text(finalize_page(template), encoding="utf-8")
    css = (PAGES_DIR / "styles.css").read_text(encoding="utf-8")
    (out_dir / "styles.css").write_text(minify_css(css), encoding="utf-8")

    assets_dir = out_dir / "assets"
    assets_dir.mkdir()
    for name, source in ASSETS.items():
        dest = assets_dir / name
        shutil.copyfile(source, dest)
        if name in MODERN_FORMAT_ASSETS:
            convert_to_modern_formats(dest, assets_dir)

    shutil.copytree(PAGES_DIR / "fonts", out_dir / "fonts")
    shutil.copyfile(CNAME_FILE, out_dir / "CNAME")

    build_doc_pages(out_dir, doc_results)
    build_videos_page(out_dir)
    build_coverage_page(out_dir, coverage_dir, version)
    build_sitemap(out_dir, doc_results)
    build_robots_txt(out_dir)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=PAGES_DIR / "dist")
    parser.add_argument(
        "--version",
        default="",
        help="Version tag to display on the site (e.g. v1.2.3); shown as 'unreleased' if omitted",
    )
    parser.add_argument(
        "--coverage-dir",
        type=Path,
        default=None,
        help=(
            "Directory of downloaded lcov coverage artifacts (see .github/scripts/coverage_summary.py's "
            "MODULES) for the coverage subpage; omitted or missing shows a 'data unavailable' page instead"
        ),
    )
    args = parser.parse_args()
    build(args.out, args.version, args.coverage_dir)
    print(f"Built site into {args.out}")


if __name__ == "__main__":
    main()
