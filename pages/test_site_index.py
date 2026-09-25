"""Tests for sitemap and robots.txt generation."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import site_index


class SitemapAndRobotsTest(unittest.TestCase):
    def test_sitemap_urls_lists_the_homepage_videos_page_and_every_doc(self) -> None:
        docs = [{"slug": "guide"}, {"slug": "missing"}]
        doc_results = {"guide": {}}

        with (
            patch.object(site_index, "SITE_URL", "https://example.test/"),
            patch.object(site_index, "DOCS", docs),
        ):
            urls = site_index.sitemap_urls(doc_results)

        self.assertEqual(urls, [
            "https://example.test/", "https://example.test/action.html",
            "https://example.test/rules.html", "https://example.test/videos.html",
            "https://example.test/coverage.html", "https://example.test/imprint.html",
            "https://example.test/docs/index.html", "https://example.test/docs/guide.html",
        ])

    def test_build_sitemap_writes_escaped_urls(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            with patch.object(site_index, "SITE_URL", "https://example.test/a&b/"):
                site_index.build_sitemap(out_dir, {})
            content = (out_dir / "sitemap.xml").read_text(encoding="utf-8")

        self.assertTrue(content.startswith('<?xml version="1.0" encoding="UTF-8"?>\n'))
        self.assertIn("<loc>https://example.test/a&amp;b/</loc>", content)
        self.assertIn("<loc>https://example.test/a&amp;b/action.html</loc>", content)
        self.assertIn("<loc>https://example.test/a&amp;b/videos.html</loc>", content)

    def test_build_robots_txt_points_at_the_sitemap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            with patch.object(site_index, "SITE_URL", "https://example.test/"):
                site_index.build_robots_txt(out_dir)
            content = (out_dir / "robots.txt").read_text(encoding="utf-8")

        self.assertEqual(content, "User-agent: *\nAllow: /\n\nSitemap: https://example.test/sitemap.xml\n")
