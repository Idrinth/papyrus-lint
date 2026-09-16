"""The small subset of Markdown used by README.md/docs/*.md, converted to
HTML for the Pages builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change. This is
deliberately not a general CommonMark parser: it only supports headings,
paragraphs, fenced code blocks (handed off to pages.highlighting for
syntax highlighting), and render_inline's small set of inline formatting
(links, code spans, bold). Everything else is escaped rather than
interpreted, which is also what keeps content fetched from elsewhere (the
papyrus-lint-action README, see build.py's ACTION_DOC) from being able to
inject arbitrary HTML into the built site.
"""

from __future__ import annotations

import html
import re

try:
    from pages.highlighting import highlight_code
except ImportError:  # running as pages/build.py
    from highlighting import highlight_code

HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")
INLINE_LINK_RE = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
INLINE_CODE_RE = re.compile(r"`([^`]+)`")
INLINE_BOLD_RE = re.compile(r"\*\*([^*]+)\*\*")


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
        if not stripped:
            flush_paragraph()
            i += 1
            continue
        para.append(stripped)
        i += 1
    flush_paragraph()
    return "\n".join(out)
