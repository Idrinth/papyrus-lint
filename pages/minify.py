"""HTML, CSS, and JavaScript minification used by the Pages builder.

The previous implementation was a handful of regular expressions. Those
are replaced here by htmlmin, rcssmin, and rjsmin. <pre> blocks are still
stashed before HTML minification so example whitespace and comment-like
text inside them stay verbatim.
"""

from __future__ import annotations

import re

import htmlmin
import rcssmin
import rjsmin

PRE_BLOCK_RE = re.compile(r"<pre\b[^>]*>.*?</pre>", re.DOTALL | re.IGNORECASE)
IMG_SOURCE_TAG_RE = re.compile(r"<(?:img|source)\b[^>]*>", re.IGNORECASE)


def minify_html(text: str) -> str:
    """Minifies static HTML for deployment via htmlmin.

    <pre>...</pre> blocks are stashed first so example whitespace and any
    comment-like text inside them stay verbatim; htmlmin would otherwise
    treat those comments as real markup comments and drop them.
    """
    blocks: list[str] = []

    def stash(match: re.Match[str]) -> str:
        blocks.append(match.group(0))
        return f"\x00{len(blocks) - 1}\x00"

    result = PRE_BLOCK_RE.sub(stash, text)
    result = htmlmin.minify(
        result,
        remove_comments=True,
        remove_empty_space=True,
        remove_optional_attribute_quotes=False,
        reduce_empty_attributes=False,
        convert_charrefs=False,
        keep_pre=True,
    ).strip()
    result = IMG_SOURCE_TAG_RE.sub(_keep_void_slash, result)
    result = re.sub(r"<(link|meta)\b([^>]*?)\s*/>", r"<\1\2>", result)
    return re.sub(r"\x00(\d+)\x00", lambda m: blocks[int(m.group(1))], result)


def _keep_void_slash(match: re.Match[str]) -> str:
    """htmlmin emits HTML5 void tags; keep the self-closing slash this
    builder writes on <img> and <source> tags."""
    tag = match.group(0)
    if tag.endswith("/>"):
        return tag
    return f"{tag[:-1].rstrip()} />"


def minify_css(text: str) -> str:
    """Minifies CSS for deployment via rcssmin."""
    return rcssmin.cssmin(text)


def minify_js(text: str) -> str:
    """Minifies JavaScript for deployment via rjsmin."""
    return rjsmin.jsmin(text)
