"""Tests for the GitHub Pages site builder's own orchestration: the
homepage, videos/imprint pages, the sitemap/robots.txt, and the full
build() pipeline that assembles every subpage. Concern-specific pieces
extracted out of pages/build.py have their own dedicated test modules:
pages/test_css.py, pages/test_site_assets.py, pages/test_site_chrome.py,
pages/test_docs_pages.py, pages/test_rules_page.py, and (for the coverage
subpage) pages/test_coverage_report.py.
"""

from __future__ import annotations

import json
import runpy
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import MagicMock, patch

from PIL import Image

from pages import build as page_builder
from pages import docs_pages, rules_page, site_assets, site_chrome


class RenderVideosListTest(unittest.TestCase):
    def test_render_videos_list_embeds_each_video_and_escapes_title(self) -> None:
        result = page_builder.render_videos_list(
            [{"id": 'abc123?feature="test"&safe=yes', "title": '1.0.0 <overview> & "tour"'}]
        )

        self.assertIn(
            'src="https://www.youtube-nocookie.com/embed/abc123?feature=&quot;test&quot;&amp;safe=yes"',
            result,
        )
        escaped_title = "1.0.0 &lt;overview&gt; &amp; &quot;tour&quot;"
        self.assertIn(f'title="{escaped_title}"', result)
        self.assertIn(f"<figcaption>{escaped_title}</figcaption>", result)
        self.assertNotIn("<overview>", result)

    def test_render_videos_list_preserves_input_order_and_card_structure(self) -> None:
        result = page_builder.render_videos_list(
            [
                {"id": "old-video", "title": "Old walkthrough"},
                {"id": "new-video", "title": "New walkthrough"},
            ]
        )

        self.assertLess(result.index("old-video"), result.index("new-video"))
        self.assertEqual(result.count('<figure class="video-card">'), 2)
        self.assertEqual(result.count('loading="lazy"'), 2)
        self.assertEqual(result.count("allowfullscreen"), 2)

    def test_render_videos_list_handles_an_empty_catalog(self) -> None:
        self.assertEqual(page_builder.render_videos_list([]), "")


class VideosPageTest(unittest.TestCase):
    def test_build_videos_page_loads_json_and_replaces_the_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            videos_file = pages_dir / "videos.json"
            videos_file.write_text(
                '[{"id": "first", "title": "First"}, '
                '{"id": "second", "title": "Second"}]',
                encoding="utf-8",
            )
            (pages_dir / "videos.template.html").write_text(
                "<main>before<!--VIDEOS_LIST-->after</main>", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "VIDEOS_FILE", videos_file),
            ):
                page_builder.build_videos_page(out_dir)

            output = (out_dir / "videos.html").read_text(encoding="utf-8")
            self.assertNotIn("<!--VIDEOS_LIST-->", output)
            self.assertIn("<main>before", output)
            self.assertIn("after</main>", output)
            self.assertLess(output.index("First"), output.index("Second"))
            self.assertEqual(output.count("youtube-nocookie.com/embed/"), 2)

    def test_build_videos_page_rejects_a_template_without_the_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            videos_file = pages_dir / "videos.json"
            videos_file.write_text("[]", encoding="utf-8")
            (pages_dir / "videos.template.html").write_text(
                "<main>No marker</main>", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "VIDEOS_FILE", videos_file),
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
                page_builder.build_videos_page(out_dir)

            self.assertFalse((out_dir / "videos.html").exists())

    def test_build_videos_page_escapes_the_version_in_shared_components(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            includes_dir = pages_dir / "includes"
            out_dir = root / "out"
            includes_dir.mkdir(parents=True)
            out_dir.mkdir()
            videos_file = pages_dir / "videos.json"
            videos_file.write_text("[]", encoding="utf-8")
            (pages_dir / "videos.template.html").write_text(
                "<!--SITE_HEADER--><main><!--VIDEOS_LIST--></main><!--SITE_FOOTER-->",
                encoding="utf-8",
            )
            (includes_dir / "header.html").write_text("<header>Videos</header>", encoding="utf-8")
            (includes_dir / "footer.html").write_text(
                "<footer><!--VERSION--></footer>", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "VIDEOS_FILE", videos_file),
                patch.object(site_chrome, "INCLUDES_DIR", includes_dir),
                patch.object(site_chrome, "render_funding_links", return_value=""),
            ):
                page_builder.build_videos_page(out_dir, 'v2<&"')

            output = (out_dir / "videos.html").read_text(encoding="utf-8")

        self.assertIn("<header>Videos</header>", output)
        self.assertIn("<footer>v2&lt;&amp;&quot;</footer>", output)


class ImprintPageTest(unittest.TestCase):
    def test_build_imprint_page_applies_shared_chrome(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            includes_dir = pages_dir / "includes"
            out_dir = root / "out"
            includes_dir.mkdir(parents=True)
            out_dir.mkdir()
            (pages_dir / "imprint.template.html").write_text(
                "<!--SITE_HEADER--><main>Legal Notice</main><!--SITE_FOOTER-->",
                encoding="utf-8",
            )
            (includes_dir / "header.html").write_text("<header>Imprint</header>", encoding="utf-8")
            (includes_dir / "footer.html").write_text(
                "<footer><!--VERSION--></footer>", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(site_chrome, "INCLUDES_DIR", includes_dir),
                patch.object(site_chrome, "render_funding_links", return_value=""),
            ):
                page_builder.build_imprint_page(out_dir, 'v2<&"')

            output = (out_dir / "imprint.html").read_text(encoding="utf-8")

        self.assertIn("<header>Imprint</header>", output)
        self.assertIn("Legal Notice", output)
        self.assertIn("<footer>v2&lt;&amp;&quot;</footer>", output)


class RepositoryConfigurationTest(unittest.TestCase):
    """Keep build.py's checked-in inputs synchronized with its manifest data."""

    def test_video_catalog_has_unique_nonempty_ids_and_titles(self) -> None:
        videos = page_builder.json.loads(page_builder.VIDEOS_FILE.read_text(encoding="utf-8"))
        ids = [video["id"] for video in videos]
        titles = [video["title"] for video in videos]

        self.assertTrue(videos)
        self.assertEqual(len(ids), len(set(ids)), "YouTube video IDs must be unique")
        self.assertEqual(len(titles), len(set(titles)), "YouTube video titles must be unique")
        for video in videos:
            with self.subTest(video=video):
                self.assertTrue(video["id"].strip())
                self.assertTrue(video["title"].strip())

    def test_page_templates_have_complete_shared_chrome_and_required_markers(self) -> None:
        required_markers = {
            "index.template.html": {"<!--CLI_EXAMPLES-->", "<!--DOCS_LIST-->", "<!--CONTACT-LINKS-->"},
            "videos.template.html": {"<!--VIDEOS_LIST-->"},
            "action.template.html": {
                "<!--ACTION_TITLE-->",
                "<!--ACTION_DESCRIPTION-->",
                "<!--ACTION_CONTENT-->",
            },
            "coverage.template.html": {"<!--COVERAGE_VERSION-->", "<!--COVERAGE_CONTENT-->"},
            "imprint.template.html": set(),
            "docs.template.html": {
                "<!--DOC_TITLE-->",
                "<!--DOC_DESCRIPTION-->",
                "<!--DOC_URL-->",
                "<!--DOC_CONTENT-->",
            },
        }

        for filename, markers in required_markers.items():
            template = (page_builder.PAGES_DIR / filename).read_text(encoding="utf-8")
            with self.subTest(template=filename):
                self.assertIn("<!--SITE_HEADER-->", template)
                self.assertIn("<!--SITE_FOOTER-->", template)
                for marker in markers:
                    self.assertIn(marker, template)

    def test_homepage_thanks_every_named_contributor(self) -> None:
        template = (page_builder.PAGES_DIR / "index.template.html").read_text(encoding="utf-8")

        self.assertIn('<section id="thanks">', template)
        for contributor in (
            "WraithFallen",
            "Scrivener07",
            "s3ngine",
            "wall416",
            "DavidJCobb",
            "Vict",
        ):
            with self.subTest(contributor=contributor):
                self.assertIn(contributor, template)
        self.assertIn('href="https://x.com/VictMangle"', template)


class SitemapAndRobotsTest(unittest.TestCase):
    def test_sitemap_urls_lists_the_homepage_videos_page_and_every_doc(self) -> None:
        docs = [{"slug": "guide"}, {"slug": "missing"}]
        doc_results = {"guide": {}}

        with (
            patch.object(page_builder, "SITE_URL", "https://example.test/"),
            patch.object(page_builder, "DOCS", docs),
        ):
            urls = page_builder.sitemap_urls(doc_results)

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

    def test_build_sitemap_writes_escaped_urls(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            with patch.object(page_builder, "SITE_URL", "https://example.test/a&b/"):
                page_builder.build_sitemap(out_dir, {})

            content = (out_dir / "sitemap.xml").read_text(encoding="utf-8")

        self.assertTrue(content.startswith('<?xml version="1.0" encoding="UTF-8"?>\n'))
        self.assertIn("<loc>https://example.test/a&amp;b/</loc>", content)
        self.assertIn("<loc>https://example.test/a&amp;b/action.html</loc>", content)
        self.assertIn("<loc>https://example.test/a&amp;b/videos.html</loc>", content)

    def test_build_robots_txt_points_at_the_sitemap(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            with patch.object(page_builder, "SITE_URL", "https://example.test/"):
                page_builder.build_robots_txt(out_dir)

            content = (out_dir / "robots.txt").read_text(encoding="utf-8")

        self.assertEqual(
            content,
            "User-agent: *\nAllow: /\n\nSitemap: https://example.test/sitemap.xml\n",
        )


class RepositoryBuildIntegrationTest(unittest.TestCase):
    """Exercise the complete builder against the repository's real inputs."""

    def test_real_site_build_renders_every_page_and_asset(self) -> None:
        action_response = MagicMock()
        action_response.__enter__.return_value.read.return_value = (
            b"# Papyrus Lint Action\n\nLint Papyrus projects in GitHub Actions.\n"
        )

        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory) / "site"
            with patch.object(docs_pages, "urlopen", return_value=action_response):
                page_builder.build(out_dir, version="v9.8.7")

            expected_html = {
                "index.html",
                "action.html",
                "rules.html",
                "videos.html",
                "coverage.html",
                "imprint.html",
                "docs/index.html",
                *(f"{docs_pages.doc_url_prefix(doc)}/{doc['slug']}.html" for doc in page_builder.DOCS),
            }
            built_html = {
                path.relative_to(out_dir).as_posix() for path in out_dir.rglob("*.html")
            }
            self.assertEqual(built_html, expected_html)

            for relative_path in sorted(expected_html):
                with self.subTest(page=relative_path):
                    output = (out_dir / relative_path).read_text(encoding="utf-8")
                    self.assertIn("<!doctype html>", output.lower())
                    self.assertIn("v9.8.7", output)
                    self.assertNotIn("<!--", output)

            index = (out_dir / "index.html").read_text(encoding="utf-8")
            self.assertIn('href=rules.html', index)
            self.assertIn("discord.gg/idrinth", index)
            self.assertNotIn("CONTACT-LINKS", index)
            for doc in page_builder.DOCS:
                with self.subTest(homepage_doc=doc["slug"]):
                    self.assertIn(f'href={docs_pages.doc_href(doc, None)}', index)

            docs_index = (out_dir / "docs" / "index.html").read_text(encoding="utf-8")
            self.assertIn('href=../index.html#top', docs_index)
            self.assertIn('src=../theme.js', docs_index)
            self.assertIn('href=../styles.css', docs_index)
            for doc in page_builder.DOCS:
                with self.subTest(docs_index_doc=doc["slug"]):
                    self.assertIn(f'href={docs_pages.doc_href(doc, "docs")}', docs_index)

            expected_schemas = {
                path.name: path.read_bytes()
                for path in site_assets.SCHEMA_DIR.glob(site_assets.SCHEMA_GLOB)
            }
            expected_schemas[site_assets.AI_EXPORT_LEGACY_SCHEMA] = (
                site_assets.SCHEMA_DIR / site_assets.AI_EXPORT_V1_SCHEMA
            ).read_bytes()
            published_schemas = {
                path.name: path.read_bytes() for path in (out_dir / "schema").glob("*.json")
            }
            self.assertEqual(published_schemas, expected_schemas)

            self.assertEqual(
                (out_dir / "CNAME").read_text(encoding="utf-8"),
                site_chrome.CNAME_FILE.read_text(encoding="utf-8"),
            )
            css = (out_dir / "styles.css").read_text(encoding="utf-8")
            self.assertNotIn("@import", css)
            self.assertIn("--color-accent:", css)
            self.assertIn("--font-display:", css)
            self.assertIn(".button--primary", css)
            self.assertIn("@font-face", css)
            for output_name in site_assets.ASSETS:
                with self.subTest(asset=output_name):
                    self.assertTrue((out_dir / "assets" / output_name).is_file())
            for output_name in site_assets.MODERN_FORMAT_ASSETS:
                stem = Path(output_name).stem
                with self.subTest(modern_asset=output_name):
                    self.assertTrue((out_dir / "assets" / f"{stem}.webp").is_file())
                    self.assertTrue((out_dir / "assets" / f"{stem}.avif").is_file())

            self.assertTrue((out_dir / "rules.js").is_file())
            rules = rules_page.load_rules()
            rules_output = (out_dir / "rules.html").read_text(encoding="utf-8")
            self.assertIn(str(len(rules)), rules_output)
            for rule in rules:
                with self.subTest(rule=rule["id"]):
                    self.assertIn(f'id=rule-{rule["id"]}', rules_output)


class BuildTest(unittest.TestCase):
    def test_script_entry_point_displays_command_line_help(self) -> None:
        output = StringIO()

        with (
            patch("sys.argv", [str(page_builder.__file__), "--help"]),
            patch("sys.stdout", output),
            self.assertRaisesRegex(SystemExit, "0"),
        ):
            runpy.run_path(str(page_builder.__file__), run_name="__main__")

        help_text = output.getvalue()
        self.assertIn("usage:", help_text)
        self.assertIn("--coverage-dir", help_text)

    def test_build_replaces_content_copies_assets_and_cleans_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            docs_dir = root / "docs"
            docs_dir.mkdir()
            (root / "README.md").write_text(
                """## Command-line interface""",
                encoding="utf-8",
            )
            (docs_dir / "papyrus-cli-usage.txt").write_text(
                "PapyrusLinterCLI fix",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<main><!--CLI_EXAMPLES--><!--DOCS_LIST--><!--VERSION-->"
                '<img src="assets/screenshot.png" alt="Screenshot" /></main>',
                encoding="utf-8",
            )
            (pages_dir / "videos.template.html").write_text(
                "<main><!--VIDEOS_LIST--></main>", encoding="utf-8"
            )
            (pages_dir / "docs.template.html").write_text(
                "<!--DOC_TITLE--><!--DOC_DESCRIPTION--><!--DOC_CONTENT-->",
                encoding="utf-8",
            )
            (pages_dir / "coverage.template.html").write_text(
                "<!--COVERAGE_VERSION--><!--COVERAGE_CONTENT-->", encoding="utf-8"
            )
            (pages_dir / "action.template.html").write_text(
                "<!--ACTION_TITLE--><!--ACTION_DESCRIPTION--><!--ACTION_CONTENT-->",
                encoding="utf-8",
            )
            (pages_dir / "imprint.template.html").write_text(
                "<main>Legal Notice</main>", encoding="utf-8"
            )
            (pages_dir / "rules.template.html").write_text(
                "<main><!--RULES_CONTENT--></main>", encoding="utf-8"
            )
            (pages_dir / "styles.css").write_text("main { color: red; }", encoding="utf-8")
            (pages_dir / "theme.js").write_text("window.themeReady = true;", encoding="utf-8")
            (pages_dir / "downloads.js").write_text("window.downloadsReady = true;", encoding="utf-8")
            (pages_dir / "rules.js").write_text("window.rulesReady = true;", encoding="utf-8")
            rules_file = root / "rules.json"
            rules_file.write_text(
                json.dumps(
                    [
                        {
                            "id": "example-rule",
                            "name": "Example rule",
                            "tags": ["style"],
                            "severity": "info",
                            "fixable": False,
                            "description": "An example rule.",
                            "definition": "The full behavior of this rule.",
                        }
                    ]
                ),
                encoding="utf-8",
            )
            fonts_dir = pages_dir / "fonts"
            fonts_dir.mkdir()
            (fonts_dir / "font.woff2").write_bytes(b"font bytes")
            copied_asset = root / "source.png"
            copied_asset.write_bytes(b"image bytes")
            screenshot_asset = root / "screenshot-source.png"
            Image.new("RGB", (4, 4), (1, 2, 3)).save(screenshot_asset)
            out_dir = root / "public"
            out_dir.mkdir()
            (out_dir / "stale.txt").write_text("remove me", encoding="utf-8")

            action_response = MagicMock()
            action_response.__enter__.return_value.read.return_value = (
                b"# Papyrus Lint Action\n\nLints pull requests automatically.\n"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "DOCS", []),
                patch.object(docs_pages, "DOCS", []),
                patch.object(docs_pages, "PAGES_DIR", pages_dir),
                patch.object(docs_pages, "urlopen", return_value=action_response),
                patch.object(rules_page, "PAGES_DIR", pages_dir),
                patch.object(rules_page, "RULES_FILE", rules_file),
                patch.object(
                    site_assets,
                    "ASSETS",
                    {"copied.png": copied_asset, "screenshot.png": screenshot_asset},
                ),
                patch.object(page_builder, "ASSETS", {"copied.png": copied_asset, "screenshot.png": screenshot_asset}),
                patch.object(site_assets, "MODERN_FORMAT_ASSETS", {"screenshot.png"}),
                patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"screenshot.png"}),
            ):
                page_builder.build(out_dir, version="v1.2.3")

            output = (out_dir / "index.html").read_text(encoding="utf-8")
            self.assertIn("v1.2.3", output)
            self.assertNotIn("<!--CLI_EXAMPLES-->", output)
            self.assertNotIn("<!--DOCS_LIST-->", output)
            self.assertNotIn("<!--VERSION-->", output)
            self.assertIn(
                '<picture><source srcset=assets/screenshot.avif type=image/avif>'
                '<source srcset=assets/screenshot.webp type=image/webp>'
                '<img src=assets/screenshot.png alt=Screenshot></picture>',
                output,
            )
            self.assertEqual(
                (out_dir / "styles.css").read_text(encoding="utf-8"),
                "main{color:red}",
            )
            self.assertEqual((out_dir / "assets" / "copied.png").read_bytes(), b"image bytes")
            self.assertFalse((out_dir / "assets" / "copied.webp").exists())
            self.assertTrue((out_dir / "assets" / "screenshot.webp").exists())
            self.assertTrue((out_dir / "assets" / "screenshot.avif").exists())
            self.assertEqual((out_dir / "fonts" / "font.woff2").read_bytes(), b"font bytes")
            self.assertIn("themeReady", (out_dir / "theme.js").read_text(encoding="utf-8"))
            self.assertIn("downloadsReady", (out_dir / "downloads.js").read_text(encoding="utf-8"))
            self.assertIn("rulesReady", (out_dir / "rules.js").read_text(encoding="utf-8"))
            self.assertFalse((out_dir / "stale.txt").exists())
            self.assertTrue((out_dir / "docs" / "index.html").exists())

            videos_output = (out_dir / "videos.html").read_text(encoding="utf-8")
            self.assertIn("youtube-nocookie.com/embed/", videos_output)
            self.assertNotIn("<!--VIDEOS_LIST-->", videos_output)

            action_output = (out_dir / "action.html").read_text(encoding="utf-8")
            self.assertIn("Papyrus Lint Action", action_output)
            self.assertIn("Lints pull requests automatically.", action_output)
            self.assertNotIn("<!--ACTION_TITLE-->", action_output)

            rules_output = (out_dir / "rules.html").read_text(encoding="utf-8")
            self.assertIn("Example rule", rules_output)
            self.assertNotIn("<!--RULES_CONTENT-->", rules_output)

            self.assertEqual(
                (out_dir / "CNAME").read_text(encoding="utf-8"),
                site_chrome.CNAME_FILE.read_text(encoding="utf-8"),
            )

            robots_output = (out_dir / "robots.txt").read_text(encoding="utf-8")
            self.assertIn("Allow: /", robots_output)
            self.assertIn(f"Sitemap: {page_builder.SITE_URL}sitemap.xml", robots_output)

            sitemap_output = (out_dir / "sitemap.xml").read_text(encoding="utf-8")
            self.assertIn(f"<loc>{page_builder.SITE_URL}</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}action.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}rules.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}videos.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}coverage.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}imprint.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}docs/index.html</loc>", sitemap_output)

            coverage_output = (out_dir / "coverage.html").read_text(encoding="utf-8")
            self.assertIn("v1.2.3", coverage_output)
            self.assertIn("Coverage data isn't available for this build.", coverage_output)

            imprint_output = (out_dir / "imprint.html").read_text(encoding="utf-8")
            self.assertIn("Legal Notice", imprint_output)

    def test_build_rejects_a_missing_cli_examples_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            docs_dir = root / "docs"
            docs_dir.mkdir()
            (root / "README.md").write_text(
                """## Command-line interface
```
command
```
""",
                encoding="utf-8",
            )
            (docs_dir / "papyrus-cli-usage.txt").write_text(
                "PapyrusLinterCLI fix",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<!--DOCS_LIST-->", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "DOCS", []),
                patch.object(docs_pages, "DOCS", []),
                self.assertRaisesRegex(SystemExit, "missing marker <!--CLI_EXAMPLES-->"),
            ):
                page_builder.build(root / "out")

            self.assertFalse((root / "out").exists())

    def test_build_rejects_a_missing_docs_list_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            docs_dir = root / "docs"
            docs_dir.mkdir()
            (root / "README.md").write_text(
                """## Command-line interface
```
command
```
""",
                encoding="utf-8",
            )
            (docs_dir / "papyrus-cli-usage.txt").write_text(
                "PapyrusLinterCLI fix",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<!--CLI_EXAMPLES-->", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "DOCS", []),
                patch.object(docs_pages, "DOCS", []),
                self.assertRaisesRegex(SystemExit, "missing marker <!--DOCS_LIST-->"),
            ):
                page_builder.build(root / "out")

            self.assertFalse((root / "out").exists())

    def test_main_uses_default_version_and_reports_output_directory(self) -> None:
        output_dir = Path("custom-output")

        with (
            patch("sys.argv", ["build.py", "--out", str(output_dir)]),
            patch.object(page_builder, "build") as build,
            patch("sys.stdout", new_callable=StringIO) as stdout,
        ):
            page_builder.main()

        build.assert_called_once_with(output_dir, "", None)
        self.assertEqual(stdout.getvalue(), f"Built site into {output_dir}\n")

    def test_main_uses_the_pages_dist_directory_when_out_is_omitted(self) -> None:
        with (
            patch("sys.argv", ["build.py"]),
            patch.object(page_builder, "build") as build,
            patch("sys.stdout", new_callable=StringIO) as stdout,
        ):
            page_builder.main()

        expected_output = page_builder.PAGES_DIR / "dist"
        build.assert_called_once_with(expected_output, "", None)
        self.assertEqual(stdout.getvalue(), f"Built site into {expected_output}\n")

    def test_main_passes_an_explicit_version_to_build(self) -> None:
        output_dir = Path("versioned-output")

        with (
            patch("sys.argv", ["build.py", "--out", str(output_dir), "--version", "v9.8.7"]),
            patch.object(page_builder, "build") as build,
            patch("sys.stdout", new_callable=StringIO),
        ):
            page_builder.main()

        build.assert_called_once_with(output_dir, "v9.8.7", None)

    def test_main_passes_a_coverage_dir_to_build(self) -> None:
        output_dir = Path("coverage-output")
        coverage_dir = Path("coverage-artifacts")

        with (
            patch(
                "sys.argv",
                ["build.py", "--out", str(output_dir), "--coverage-dir", str(coverage_dir)],
            ),
            patch.object(page_builder, "build") as build,
            patch("sys.stdout", new_callable=StringIO),
        ):
            page_builder.main()

        build.assert_called_once_with(output_dir, "", coverage_dir)

    def test_main_passes_version_and_coverage_options_together(self) -> None:
        output_dir = Path("complete-output")
        coverage_dir = Path("downloaded-coverage")

        with (
            patch(
                "sys.argv",
                [
                    "build.py",
                    "--out",
                    str(output_dir),
                    "--version",
                    "v4.5.6",
                    "--coverage-dir",
                    str(coverage_dir),
                ],
            ),
            patch.object(page_builder, "build") as build,
            patch("sys.stdout", new_callable=StringIO) as stdout,
        ):
            page_builder.main()

        build.assert_called_once_with(output_dir, "v4.5.6", coverage_dir)
        self.assertEqual(stdout.getvalue(), f"Built site into {output_dir}\n")


if __name__ == "__main__":
    unittest.main()
