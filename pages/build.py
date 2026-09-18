#!/usr/bin/env python3
"""Builds the GitHub Pages site from the templates under pages/.

Substitutes the CLI usage examples in the template with content converted
directly from README.md's own code blocks, so that documentation never has
to be kept in sync by hand in two places.
Also renders every document listed in pages/docs_pages.py's DOCS (including
remotely sourced documentation) into its own browsable subpage under docs/
(via pages/docs.template.html), with lightweight build-time syntax
highlighting (pages/highlighting.py) for fenced Markdown (pages/markdown_render.py,
including Papyrus code fences) and raw JSON, YAML, shell, and BBCode
sources. The
templates receive their
shared header and footer from pages/includes/, so site chrome has a single
source of truth - including the header's System/Light/Dark theme switch,
wired up by pages/theme.js (minified into the output directory) and
backed by a small blocking inline script duplicated into each template's
own <head> (before the stylesheet link, to set an already-persisted
light/dark override ahead of first paint and avoid a flash of the wrong
theme), the same "system"/"light"/"dark" scheme and localStorage key the
desktop app's own theme switch uses. The homepage's hero "Download
GUI"/"Download CLI" buttons are progressively enhanced the same way, by
pages/downloads.js (also minified): without it they're plain links
to the latest GitHub release page, encoded via each button's own
data-options attribute; with it, clicking one opens a quick-select panel
of that release's actual per-platform assets instead, pre-selected by the
OS the browser reports. The builder also assembles the site's
assets/ directory
(see pages/site_assets.py) by copying the screenshots and icon this page
uses from resources/ and app/src-tauri/icons, rather than committing
duplicate copies of them under pages/. Every asset actually rendered as
an <img> also gets a WebP and an AVIF sibling (see
site_assets.convert_to_modern_formats), and every such <img> tag, in every
generated page, is rewritten into a <picture> offering those smaller
formats ahead of the original as a fallback (see
site_assets.wrap_images_with_modern_sources, applied to every page by
pages/site_chrome.py's finalize_page).
Every generated HTML page, the stylesheet, and the site scripts are minified
(see pages/minify.py) before being written into the output directory.
`pages/styles.css` imports the shared visual identity from
`shared/theme.css`; those `@import`s are inlined (see
pages/css.py's inline_css_imports) so the deployed site still ships a
single stylesheet.
Also renders action.html (via pages/action.template.html) and every
docs/ subpage (via pages/docs.template.html) - see pages/docs_pages.py.
Also renders rules.html (via pages/rules.template.html) - a searchable/
filterable reference of every lint rule generated straight from
docs/rules.json's own metadata (id, severity, tags, auto-fix support, full
documented behavior) - see pages/rules_page.py; the homepage itself only
links to it rather than duplicating any of that content.
Also renders coverage.html (via pages/coverage.template.html), a per-module,
per-file line coverage breakdown built from a directory of downloaded lcov
reports passed as --coverage-dir - see pages/coverage_report.py, reusing
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
import json
import shutil
from pathlib import Path

try:
    from pages.coverage_report import build_coverage_page
    from pages.css import inline_css_imports
    from pages.docs_pages import DOCS, build_action_page, build_doc_pages, render_doc, render_docs_list_items
    from pages.markdown_render import extract_section, first_code_block
    from pages.minify import minify_css, minify_js
    from pages.rules_page import build_rules_page
    from pages.site_assets import ASSETS, MODERN_FORMAT_ASSETS, convert_to_modern_formats, copy_json_schemas
    from pages.site_chrome import CNAME_FILE, SITE_URL, finalize_page, render_shared_components
except ImportError:  # running as pages/build.py
    from coverage_report import build_coverage_page
    from css import inline_css_imports
    from docs_pages import DOCS, build_action_page, build_doc_pages, render_doc, render_docs_list_items
    from markdown_render import extract_section, first_code_block
    from minify import minify_css, minify_js
    from rules_page import build_rules_page
    from site_assets import ASSETS, MODERN_FORMAT_ASSETS, convert_to_modern_formats, copy_json_schemas
    from site_chrome import CNAME_FILE, SITE_URL, finalize_page, render_shared_components

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent

# Simple list of YouTube video IDs/titles rendered onto videos.html, so a new
# video can be added without touching build.py or its template.
VIDEOS_FILE = PAGES_DIR / "videos.json"


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


def build_videos_page(out_dir: Path, version: str = "") -> None:
    videos = json.loads(VIDEOS_FILE.read_text(encoding="utf-8"))
    template = (PAGES_DIR / "videos.template.html").read_text(encoding="utf-8")
    if "<!--VIDEOS_LIST-->" not in template:
        raise SystemExit("videos.template.html: missing marker <!--VIDEOS_LIST-->")
    page = template.replace("<!--VIDEOS_LIST-->", render_videos_list(videos))
    page = render_shared_components(page, "", version)
    (out_dir / "videos.html").write_text(finalize_page(page), encoding="utf-8")


def build_imprint_page(out_dir: Path, version: str = "") -> None:
    """Renders the fully static legal-notice page (no build-time content of
    its own to substitute in, unlike every other page above) into
    imprint.html, so it still shares the site's header/footer/version chrome
    like every other page."""
    template = (PAGES_DIR / "imprint.template.html").read_text(encoding="utf-8")
    page = render_shared_components(template, "", version)
    (out_dir / "imprint.html").write_text(finalize_page(page), encoding="utf-8")


def sitemap_urls(doc_results: dict) -> list[str]:
    """Every page build() renders, as absolute SITE_URL-rooted URLs, in the
    same order sitemap.xml lists them. Kept in one place so the sitemap can
    never drift from the pages actually published."""
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
    cli_examples = html.escape(Path("/docs/papyrus-cli-usage.txt").read_text(encoding="utf-8", errors="replace"))

    doc_results = {}
    for doc in DOCS:
        title, description, content_html = render_doc(doc)
        doc_results[doc["slug"]] = {"title": title, "description": description, "content_html": content_html}

    template = (PAGES_DIR / "index.template.html").read_text(encoding="utf-8")
    cli_marker = "<!--CLI_EXAMPLES-->"
    if cli_marker not in template:
        raise SystemExit(f"index.template.html: missing marker {cli_marker}")
    template = template.replace(
        cli_marker, f'<pre class="code-block cli-examples"><code>{cli_examples}</code></pre>'
    )
    if "<!--DOCS_LIST-->" not in template:
        raise SystemExit("index.template.html: missing marker <!--DOCS_LIST-->")
    template = template.replace("<!--DOCS_LIST-->", render_docs_list_items(doc_results, "docs/"))
    template = render_shared_components(template, "", version)

    if out_dir.exists():
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True)
    (out_dir / "index.html").write_text(finalize_page(template), encoding="utf-8")
    css_path = PAGES_DIR / "styles.css"
    css = inline_css_imports(css_path.read_text(encoding="utf-8"), css_path)
    (out_dir / "styles.css").write_text(minify_css(css), encoding="utf-8")
    (out_dir / "theme.js").write_text(
        minify_js((PAGES_DIR / "theme.js").read_text(encoding="utf-8")),
        encoding="utf-8",
    )
    (out_dir / "downloads.js").write_text(
        minify_js((PAGES_DIR / "downloads.js").read_text(encoding="utf-8")),
        encoding="utf-8",
    )
    (out_dir / "rules.js").write_text(
        minify_js((PAGES_DIR / "rules.js").read_text(encoding="utf-8")),
        encoding="utf-8",
    )

    assets_dir = out_dir / "assets"
    assets_dir.mkdir()
    for name, source in ASSETS.items():
        dest = assets_dir / name
        shutil.copyfile(source, dest)
        if name in MODERN_FORMAT_ASSETS:
            convert_to_modern_formats(dest, assets_dir)

    shutil.copytree(PAGES_DIR / "fonts", out_dir / "fonts")
    shutil.copyfile(CNAME_FILE, out_dir / "CNAME")

    build_doc_pages(out_dir, doc_results, version)
    copy_json_schemas(out_dir)
    build_videos_page(out_dir, version)
    build_action_page(out_dir, version)
    build_rules_page(out_dir, version)
    build_coverage_page(out_dir, coverage_dir, version)
    build_imprint_page(out_dir, version)
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
