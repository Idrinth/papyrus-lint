"""The small subset of Markdown used by README.md/docs/*.md, converted to
HTML for the Pages builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change. This is
deliberately not a general CommonMark parser: it only supports headings,
paragraphs, fenced code blocks (handed off to pages.highlighting for
syntax highlighting), flat unordered lists, and render_inline's small set
of inline formatting (links, code spans, bold). Everything else is
escaped rather than interpreted, which is also what keeps content fetched
from elsewhere (the papyrus-lint-action README, see build.py's
ACTION_DOC) from being able to inject arbitrary HTML into the built site.
"""

from __future__ import annotations

import html
import re

try:
    from pages.highlighting import highlight_code
except ImportError:  # running as pages/build.py
    from highlighting import highlight_code

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
LIST_ITEM_RE = re.compile(r"^-\s+(.*)$")
INLINE_LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
INLINE_CODE_RE = re.compile(r"`([^`]+)`")
INLINE_BOLD_RE = re.compile(r"\*\*([^*]+)\*\*")


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


def markdown_to_html(lines: list[str], link_rewrite=None) -> str:
    """Converts the small subset of Markdown used by docs/*.md (headings,
    paragraphs, fenced code blocks, flat unordered lists, and
    render_inline's inline formatting) into HTML."""
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
            language = stripped[3:].strip().split(maxsplit=1)[0] if stripped[3:].strip() else None
            i += 1
            code_lines: list[str] = []
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code_lines.append(lines[i])
                i += 1
            i += 1
            code_html = highlight_code(chr(10).join(code_lines), language)
            language_class = f" language-{html.escape(language, quote=True)}" if language else ""
            out.append(
                f'<pre class="code-block{language_class}" tabindex="0"><code>{code_html}</code></pre>'
            )
            continue
        heading = HEADING_RE.match(line)
        if heading:
            flush_paragraph()
            level = len(heading.group(1))
            out.append(f"<h{level}>{render_inline(heading.group(2), link_rewrite)}</h{level}>")
            i += 1
            continue
        list_item = LIST_ITEM_RE.match(stripped)
        if list_item:
            flush_paragraph()
            items = [[list_item.group(1)]]
            i += 1
            while i < len(lines) and lines[i].strip():
                if HEADING_RE.match(lines[i]) or lines[i].strip().startswith("```"):
                    break
                next_stripped = lines[i].strip()
                next_item = LIST_ITEM_RE.match(next_stripped)
                if next_item:
                    items.append([next_item.group(1)])
                else:
                    items[-1].append(next_stripped)
                i += 1
            list_html = "".join(
                f"<li>{render_inline(' '.join(item_lines), link_rewrite)}</li>" for item_lines in items
            )
            out.append(f"<ul>{list_html}</ul>")
            continue
        if not stripped:
            flush_paragraph()
            i += 1
            continue
        para.append(stripped)
        i += 1
    flush_paragraph()
    return "\n".join(out)
