"""Load shared/links.yaml and render tagged subsets for build-time injection.

Labels are the mapping keys; `type` values are the tags callers filter by.
Markers encode the tag so destinations never hardcode a label:

- ``<!--CONTACT-LINKS-->`` / ``<CONTACT-LINKS>`` → entries tagged `contact`
- ``<!--DOWNLOAD-LINKS-->`` / ``<DOWNLOAD-LINKS>`` → entries tagged `download`
- ``<!--LINKS-->`` / ``<LINKS>`` → every entry
"""

from __future__ import annotations

import html
import re
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse

LINKS_FILE = Path(__file__).resolve().parents[3] / "shared" / "links.yaml"

HTML_MARKER_RE = re.compile(r"<!--(?:([A-Z][A-Z0-9]*)-)?LINKS-->")
ANGLE_MARKER_RE = re.compile(r"<(?:([A-Z][A-Z0-9]*)-)?LINKS>")


@dataclass(frozen=True)
class Link:
    label: str
    url: str
    tags: frozenset[str]


def load_links(path: Path | None = None) -> list[Link]:
    """Parse *path* (default: the repository's ``shared/links.yaml``)."""
    path = path or LINKS_FILE
    return parse_links(path.read_text(encoding="utf-8"), path=path)


def parse_links(source: str, path: Path | str | None = None) -> list[Link]:
    """Parse the links YAML subset this repository actually writes."""
    origin = str(path) if path is not None else "links.yaml"
    links: list[Link] = []
    seen: set[str] = set()
    label: str | None = None
    url: str | None = None
    tags: list[str] = []
    in_type_list = False

    def finish() -> None:
        nonlocal label, url, tags, in_type_list
        if label is None:
            return
        if not url:
            raise ValueError(f"{origin}: {label!r} is missing url")
        parsed = urlparse(url)
        if parsed.scheme not in {"http", "https"} or not parsed.netloc:
            raise ValueError(f"{origin}: {label!r} url must be an HTTP(S) URL: {url}")
        if not tags:
            raise ValueError(f"{origin}: {label!r} is missing type")
        links.append(Link(label, url, frozenset(tags)))
        label = None
        url = None
        tags = []
        in_type_list = False

    for line_number, raw_line in enumerate(source.splitlines(), start=1):
        stripped_full = raw_line.strip()
        if not stripped_full or stripped_full.startswith("#"):
            continue
        line = raw_line.rstrip()
        indent = len(line) - len(line.lstrip(" "))
        stripped = line.strip()
        location = f"{origin}:{line_number}"

        if indent == 0:
            finish()
            if not stripped.endswith(":") or stripped == ":":
                raise ValueError(f"{location}: expected 'Label:'")
            label = stripped[:-1].strip()
            if not label:
                raise ValueError(f"{location}: empty label")
            if label in seen:
                raise ValueError(f"{location}: duplicate label {label!r}")
            seen.add(label)
            continue

        if label is None:
            raise ValueError(f"{location}: indented line is not under a label")

        if stripped.startswith("- "):
            if not in_type_list:
                raise ValueError(f"{location}: list item is not under type")
            tag = stripped[2:].strip()
            if not tag:
                raise ValueError(f"{location}: empty type tag")
            if tag in tags:
                raise ValueError(f"{location}: duplicate type tag {tag!r}")
            tags.append(tag)
            continue

        key, separator, value = stripped.partition(":")
        if not separator:
            raise ValueError(f"{location}: expected 'key: value'")
        key = key.strip()
        value = value.strip()
        if key == "url":
            if not value:
                raise ValueError(f"{location}: empty url")
            url = value
            in_type_list = False
        elif key == "type":
            in_type_list = True
            if value:
                raise ValueError(f"{location}: type must be a list, not {value!r}")
        else:
            raise ValueError(f"{location}: unknown key {key!r}")

    finish()
    if not links:
        raise ValueError(f"{origin}: no links")
    return links


def filter_by_tag(links: Sequence[Link], tag: str | None) -> list[Link]:
    """Return entries tagged *tag*, or *links* unchanged when *tag* is None."""
    if tag is None:
        return list(links)
    return [link for link in links if tag in link.tags]


def render_html_anchors(links: Sequence[Link]) -> str:
    """Inline ``<a>`` tags, for a badge row or similar flow layout."""
    return "\n".join(
        f'<a href="{html.escape(link.url, quote=True)}" target="_blank" rel="noopener noreferrer">'
        f"{html.escape(link.label)}</a>"
        for link in links
    )


def render_html_list_items(links: Sequence[Link]) -> str:
    """``<li><a>`` items, for a ``<ul>`` contact list."""
    return "\n".join(
        f'<li><a href="{html.escape(link.url, quote=True)}" target="_blank" rel="noopener noreferrer">'
        f"{html.escape(link.label)}</a></li>"
        for link in links
    )


def render_bbcode_list(links: Sequence[Link]) -> str:
    """A Nexus Mods ``[list]`` of labeled ``[url]`` entries."""
    items = "".join(f"[*][url={link.url}]{link.label}[/url][/*]\n" for link in links)
    return f"[list]\n{items}[/list]"


def render_plain_text(links: Sequence[Link]) -> str:
    """Column-aligned ``label  url`` lines for CLI help."""
    width = max(len(link.label) for link in links)
    return "".join(f"  {link.label.ljust(width)}  {link.url}\n" for link in links)


def render_markdown_list_items(links: Sequence[Link]) -> str:
    """``- [Label](url)`` lines, for a Markdown contact list."""
    return "\n".join(f"- [{link.label}]({link.url})" for link in links)


def replace_html_link_markers(
    text: str,
    renderer: Callable[[Sequence[Link]], str],
    links: Sequence[Link] | None = None,
    path: Path | None = None,
) -> str:
    """Replace ``<!--TAG-LINKS-->`` / ``<!--LINKS-->`` using *renderer*."""
    return _replace_markers(HTML_MARKER_RE, text, renderer, links, path)


def replace_angle_link_markers(
    text: str,
    renderer: Callable[[Sequence[Link]], str],
    links: Sequence[Link] | None = None,
    path: Path | None = None,
) -> str:
    """Replace ``<TAG-LINKS>`` / ``<LINKS>`` using *renderer*."""
    return _replace_markers(ANGLE_MARKER_RE, text, renderer, links, path)


def _replace_markers(
    pattern: re.Pattern[str],
    text: str,
    renderer: Callable[[Sequence[Link]], str],
    links: Sequence[Link] | None,
    path: Path | None,
) -> str:
    if pattern.search(text) is None:
        return text
    loaded = list(links) if links is not None else load_links(path)

    def repl(match: re.Match[str]) -> str:
        tag = match.group(1)
        filtered = filter_by_tag(loaded, tag.lower() if tag else None)
        if not filtered:
            origin = str(path or LINKS_FILE)
            if tag is None:
                raise ValueError(f"{origin}: no links")
            raise ValueError(f"{origin}: no links tagged {tag.lower()!r}")
        return renderer(filtered)

    return pattern.sub(repl, text)
