"""Tests for sitemap and robots.txt generation."""

import builtins
import runpy
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from xml.etree import ElementTree

from pages import docs_pages, site_chrome, site_index


class SitemapAndRobotsTest(unittest.TestCase):
    def test_module_supports_the_standalone_build_script_import_path(self) -> None:
        real_import = builtins.__import__

        def import_without_pages_package(name, *args, **kwargs):
            if name in {"pages.docs_pages", "pages.site_chrome"}:
                raise ImportError("simulate pages/build.py execution")
            return real_import(name, *args, **kwargs)

        with (
            patch("builtins.__import__", side_effect=import_without_pages_package),
            patch.dict(sys.modules, {"docs_pages": docs_pages, "site_chrome": site_chrome}),
        ):
            module_globals = runpy.run_path(str(Path(site_index.__file__)))

        self.assertIs(module_globals["DOCS"], docs_pages.DOCS)
        self.assertIs(module_globals["doc_url_prefix"], docs_pages.doc_url_prefix)
        self.assertEqual(module_globals["SITE_URL"], site_chrome.SITE_URL)

    def test_sitemap_urls_lists_the_homepage_videos_page_and_every_doc(self) -> None:
        docs = [{"slug": "guide"}, {"slug": "missing"}]
        doc_results = {"guide": {}}

        with (
            patch.object(site_index, "SITE_URL", "https://example.test/"),
            patch.object(site_index, "DOCS", docs),
        ):
            urls = site_index.sitemap_urls(doc_results)

        self.assertEqual(
            urls,
            [
                "https://example.test/",
                "https://example.test/action.html",
                "https://example.test/rules.html",
                "https://example.test/videos.html",
                "https://example.test/coverage.html",
                "https://example.test/imprint.html",
                "https://example.test/docs/index.html",
                "https://example.test/docs/guide.html",
            ],
        )

    def test_sitemap_urls_preserves_doc_order_and_uses_each_docs_prefix(self) -> None:
        docs = [
            {"slug": "configuration", "repo_dir": "configuration"},
            {"slug": "guide"},
            {"slug": "schema", "repo_dir": "schema"},
        ]
        doc_results = {"schema": None, "configuration": "", "not-configured": {}}

        with (
            patch.object(site_index, "SITE_URL", "https://example.test/"),
            patch.object(site_index, "DOCS", docs),
        ):
            urls = site_index.sitemap_urls(doc_results)

        self.assertEqual(
            urls[-2:],
            [
                "https://example.test/configuration/configuration.html",
                "https://example.test/schema/schema.html",
            ],
        )
        self.assertNotIn("https://example.test/docs/guide.html", urls)

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

    def test_build_sitemap_writes_well_formed_xml_with_every_url_once(self) -> None:
        docs = [{"slug": 'guide&reference"'}]
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            with (
                patch.object(site_index, "SITE_URL", "https://example.test/"),
                patch.object(site_index, "DOCS", docs),
            ):
                expected_urls = site_index.sitemap_urls({docs[0]["slug"]: {}})
                site_index.build_sitemap(out_dir, {docs[0]["slug"]: {}})
            content = (out_dir / "sitemap.xml").read_text(encoding="utf-8")

        root = ElementTree.fromstring(content)
        namespace = {"sitemap": "http://www.sitemaps.org/schemas/sitemap/0.9"}
        locations = [element.text for element in root.findall("sitemap:url/sitemap:loc", namespace)]

        self.assertEqual(root.tag, "{http://www.sitemaps.org/schemas/sitemap/0.9}urlset")
        self.assertEqual(locations, expected_urls)
        self.assertIn("guide&amp;reference&quot;.html", content)
        self.assertTrue(content.endswith("</urlset>\n"))

    def test_build_robots_txt_points_at_the_sitemap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            with patch.object(site_index, "SITE_URL", "https://example.test/"):
                site_index.build_robots_txt(out_dir)
            content = (out_dir / "robots.txt").read_text(encoding="utf-8")

        self.assertEqual(content, "User-agent: *\nAllow: /\n\nSitemap: https://example.test/sitemap.xml\n")
