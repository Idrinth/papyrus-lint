"""Screenshot/logo/schema asset emission for the Pages builder.

Extracted out of pages/build.py (which was getting long and crowded with
unrelated site-assembly concerns) with no behavior change. Handles copying
the site's own image assets (and generating their smaller WebP/AVIF
siblings, see convert_to_modern_formats/wrap_images_with_modern_sources)
and publishing schema/*.schema.json under /schema/ (see copy_json_schemas).
"""

from __future__ import annotations

import re
import shutil
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SCHEMA_DIR = ROOT / "schema"
SCHEMA_GLOB = "*.schema.json"
AI_EXPORT_V1_SCHEMA = "papyrus-lint-ai-export.v1.schema.json"
AI_EXPORT_LEGACY_SCHEMA = "papyrus-lint-ai-export.schema.json"

ASSETS = {
    "logo-small.jpg": ROOT / "shared" / "images" / "logo-small.jpg",
    "logo.jpg": ROOT / "shared" / "images" / "logo.jpg",
    "papyrus-lint-import.png": ROOT / "shared" / "images" / "papyrus-lint-import.png",
    "papyrus-lint-results.png": ROOT / "shared" / "images" / "papyrus-lint-results.png",
    "papyrus-lint-viewer.png": ROOT / "shared" / "images" / "papyrus-lint-viewer.png",
    "papyrus-lint-vscode.png": ROOT / "shared" / "images" / "papyrus-lint-vscode.png",
    "papyrus-lint-massfix.png": ROOT / "shared" / "images" / "papyrus-lint-massfix.png",
    "papyrus-lint-cli.png": ROOT / "shared" / "images" / "papyrus-lint-cli.png",
    "favicon.png": ROOT/ "shared" / "images" / "logo.png",
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
    "papyrus-lint-massfix.png",
    "papyrus-lint-cli.png",
}

# Matches a local (not http(s), e.g. a shields.io badge) <img> tag whose src
# points into assets/ (optionally prefixed with ../, as docs/*.html pages do).
IMG_ASSET_TAG_RE = re.compile(r'<img\b[^>]*\ssrc="((?:\.\./)?assets/([\w.-]+)\.(?:png|jpg))"[^>]*/?>')


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


def copy_json_schemas(out_dir: Path) -> None:
    """Publish the checked-in schemas and the legacy AI-export URL under /schema/."""
    schema_out_dir = out_dir / "schema"
    schema_out_dir.mkdir()
    for source in sorted(SCHEMA_DIR.glob(SCHEMA_GLOB)):
        shutil.copyfile(source, schema_out_dir / source.name)
    shutil.copyfile(
        SCHEMA_DIR / AI_EXPORT_V1_SCHEMA,
        schema_out_dir / AI_EXPORT_LEGACY_SCHEMA,
    )
