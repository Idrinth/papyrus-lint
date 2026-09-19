"""Shared page chrome (header/footer/funding links) and final per-page
post-processing for the Pages builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change.
"""

from __future__ import annotations

import html
import sys
from pathlib import Path
from urllib.parse import quote, urlparse

try:
    from pages.minify import minify_html
    from pages.site_assets import wrap_images_with_modern_sources
except ImportError:  # running as pages/build.py
    from minify import minify_html
    from site_assets import wrap_images_with_modern_sources

ROOT = Path(__file__).resolve().parent.parent
PAGES_DIR = Path(__file__).resolve().parent
INCLUDES_DIR = PAGES_DIR / "includes"

CNAME_FILE = PAGES_DIR / "CNAME"
# The site's custom domain (see GitHub Pages custom domain docs) is the
# single source of truth for pages/CNAME - every absolute URL this builder
# emits (canonical/og/twitter tags, the sitemap, robots.txt) is derived from
# it rather than hardcoding the domain a second time.
SITE_URL = f"https://{CNAME_FILE.read_text(encoding='utf-8').strip()}/"

FUNDING_FILE = ROOT / ".github" / "FUNDING.yml"

FUNDING_PROVIDERS = {
    "github": ("GitHub Sponsors", "https://github.com/sponsors/{}"),
    "patreon": ("Patreon", "https://www.patreon.com/{}"),
    "open_collective": ("Open Collective", "https://opencollective.com/{}"),
    "ko_fi": ("Ko-fi", "https://ko-fi.com/{}"),
    "community_bridge": ("Community Bridge", "https://funding.communitybridge.org/projects/{}"),
    "liberapay": ("Liberapay", "https://liberapay.com/{}"),
    "issuehunt": ("IssueHunt", "https://issuehunt.io/r/{}"),
    "lfx_crowdfunding": ("LFX Crowdfunding", "https://crowdfunding.lfx.linuxfoundation.org/projects/{}"),
    "polar": ("Polar", "https://polar.sh/{}"),
    "buy_me_a_coffee": ("Buy Me a Coffee", "https://www.buymeacoffee.com/{}"),
    "thanks_dev": ("thanks.dev", "https://thanks.dev/d/{}"),
}


def _replace_link_markers(page: str) -> str:
    """Fill ``<!--TAG-LINKS-->`` / ``<!--LINKS-->`` from shared/links.yaml.

    `.github/scripts` isn't on the default import path (its directory name
    starts with a dot), so this loads ci_lib.links the same way
    coverage_report.py loads coverage_summary.py — by putting that scripts
    directory on sys.path first.
    """
    scripts_dir = str(ROOT / ".github" / "scripts")
    if scripts_dir not in sys.path:
        sys.path.insert(0, scripts_dir)
    from ci_lib.links import render_html_anchors, replace_html_link_markers

    return replace_html_link_markers(page, render_html_anchors)


def parse_funding_values(value: str) -> list[str]:
    """Parse the scalar and inline-list forms accepted by FUNDING.yml.

    GitHub's funding configuration consists only of top-level string values
    (or short inline lists), so pulling in a full YAML dependency solely for
    the site footer would be unnecessary.
    """
    value = value.strip()
    values = value[1:-1].split(",") if value.startswith("[") and value.endswith("]") else [value]
    return [item.strip().strip("'\"") for item in values if item.strip().strip("'\"")]


def render_funding_links(funding_file: Path | None = None) -> str:
    """Render footer list items from the repository's GitHub funding file."""
    funding_file = funding_file or FUNDING_FILE
    links: list[tuple[str, str]] = []
    for raw_line in funding_file.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or ":" not in line:
            continue
        provider, raw_value = line.split(":", 1)
        provider = provider.strip()
        for value in parse_funding_values(raw_value):
            if provider == "custom":
                parsed = urlparse(value)
                if parsed.scheme not in {"http", "https"} or not parsed.netloc:
                    raise SystemExit(f"{funding_file}: custom funding link must be an HTTP(S) URL: {value}")
                label = "PayPal" if parsed.hostname in {"paypal.com", "www.paypal.com"} else "Support this project"
                links.append((label, value))
            elif provider in FUNDING_PROVIDERS:
                label, url_template = FUNDING_PROVIDERS[provider]
                links.append((label, url_template.format(quote(value, safe=""))))

    return "\n".join(
        f'<li><a href="{html.escape(url, quote=True)}" target="_blank" rel="noopener noreferrer">'
        f"{html.escape(label)}</a></li>"
        for label, url in links
    )


def render_shared_components(page: str, root_path: str, version: str) -> str:
    """Insert the shared site chrome into a page template.

    ``root_path`` makes the same header work both at the site root and one
    directory down for documentation pages. Templates without either marker
    are accepted for the small, fragment-only unit-test fixtures; a real page
    with only one marker is rejected so its chrome cannot silently drift.
    """
    replacements = {
        "<!--ROOT_PATH-->": root_path,
        "<!--VERSION-->": html.escape(version) if version else "unreleased",
        "<!--FUNDING_LINKS-->": render_funding_links(),
        "<!--SITE_URL-->": SITE_URL,
    }
    for placeholder, value in replacements.items():
        page = page.replace(placeholder, value)

    markers = {"<!--SITE_HEADER-->": "header.html", "<!--SITE_FOOTER-->": "footer.html"}
    present = [marker for marker in markers if marker in page]
    if not present:
        return _replace_link_markers(page)
    if len(present) != len(markers):
        missing = next(marker for marker in markers if marker not in page)
        raise SystemExit(f"page template: missing shared component marker {missing}")

    rendered = page
    for marker, filename in markers.items():
        component = (INCLUDES_DIR / filename).read_text(encoding="utf-8")
        for placeholder, value in replacements.items():
            component = component.replace(placeholder, value)
        rendered = rendered.replace(marker, component)
    return _replace_link_markers(rendered)


def finalize_page(page_html: str) -> str:
    """Applies every page-wide HTML post-processing step - modern-format
    <picture> wrapping, then minification - shared by every full page this
    builder writes out."""
    return minify_html(wrap_images_with_modern_sources(page_html))
