"""Rendering for the site's video catalog page."""

from __future__ import annotations

import html
import json
from pathlib import Path

try:
    from pages.site_chrome import finalize_page, render_shared_components
except ImportError:  # running via pages/build.py
    from site_chrome import finalize_page, render_shared_components

PAGES_DIR = Path(__file__).resolve().parent
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
