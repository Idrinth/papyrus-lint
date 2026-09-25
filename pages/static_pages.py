"""Rendering for the site's static legal page."""

from __future__ import annotations

from pathlib import Path

try:
    from pages.site_chrome import finalize_page, render_shared_components
except ImportError:  # running via pages/build.py
    from site_chrome import finalize_page, render_shared_components

PAGES_DIR = Path(__file__).resolve().parent


def build_imprint_page(out_dir: Path, version: str = "") -> None:
    """Render the legal notice with the same shared chrome as every page."""
    template = (PAGES_DIR / "imprint.template.html").read_text(encoding="utf-8")
    page = render_shared_components(template, "", version)
    (out_dir / "imprint.html").write_text(finalize_page(page), encoding="utf-8")
