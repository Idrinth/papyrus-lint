"""Sitemap and crawler-index files for the generated site."""

from __future__ import annotations

import html
from pathlib import Path

try:
    from pages.docs_pages import DOCS, doc_url_prefix
    from pages.site_chrome import SITE_URL
except ImportError:  # running via pages/build.py
    from docs_pages import DOCS, doc_url_prefix
    from site_chrome import SITE_URL


def sitemap_urls(doc_results: dict) -> list[str]:
    """Return every published page as an absolute URL in sitemap order."""
    urls = [
        SITE_URL,
        f"{SITE_URL}action.html",
        f"{SITE_URL}rules.html",
        f"{SITE_URL}videos.html",
        f"{SITE_URL}coverage.html",
        f"{SITE_URL}imprint.html",
        f"{SITE_URL}docs/index.html",
    ]
    for doc in DOCS:
        if doc["slug"] in doc_results:
            urls.append(f"{SITE_URL}{doc_url_prefix(doc)}/{doc['slug']}.html")
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
