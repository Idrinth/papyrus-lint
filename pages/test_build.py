"""Tests for the dependency-free GitHub Pages site builder."""

from __future__ import annotations

import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from PIL import Image

from pages import build as page_builder


class MarkdownHelpersTest(unittest.TestCase):
    def test_extract_section_stops_at_same_or_shallower_heading(self) -> None:
        lines = [
            "# Guide",
            "## Wanted",
            "intro",
            "### Child",
            "child text",
            "## Next",
            "not included",
        ]

        self.assertEqual(
            page_builder.extract_section(lines, "Wanted", level=2),
            ["intro", "### Child", "child text"],
        )

    def test_extract_section_rejects_a_missing_heading(self) -> None:
        with self.assertRaisesRegex(SystemExit, "heading not found"):
            page_builder.extract_section(["## Present"], "Missing", level=2)

    def test_extract_section_ignores_same_text_at_a_different_level(self) -> None:
        lines = ["# Wanted", "wrong", "## Wanted", "right", "### Child", "also right"]

        self.assertEqual(
            page_builder.extract_section(lines, "Wanted", level=2),
            ["right", "### Child", "also right"],
        )

    def test_render_inline_converts_supported_markdown_and_escapes_html(self) -> None:
        rendered = page_builder.render_inline(
            '<unsafe> **bold** `code & more` [docs](guide.html?x=1&y=2)'
        )

        self.assertEqual(
            rendered,
            "&lt;unsafe&gt; <strong>bold</strong> "
            '<code>code &amp; more</code> '
            '<a href="guide.html?x=1&amp;y=2">docs</a>',
        )

    def test_render_inline_rewrites_and_escapes_link_targets(self) -> None:
        seen_hrefs = []

        def rewrite(href: str) -> str:
            seen_hrefs.append(href)
            return f'docs/{href}?label="read"&mode=full'

        rendered = page_builder.render_inline("[Guide](guide.md)", rewrite)

        self.assertEqual(seen_hrefs, ["guide.md"])
        self.assertEqual(
            rendered,
            '<a href="docs/guide.md?label=&quot;read&quot;&amp;mode=full">Guide</a>',
        )

    def test_split_table_row_preserves_escaped_pipes(self) -> None:
        self.assertEqual(
            page_builder.split_table_row(r"| Name | a \| b | yes |"),
            ["Name", "a | b", "yes"],
        )

    def test_render_lint_table_renders_rows_and_fix_indicator(self) -> None:
        result = page_builder.render_lint_table(
            [
                "| Lint | Description | Auto-Fix |",
                "| --- | --- | --- |",
                "| `first` | **Useful** | Yes |",
                "| second | Plain | |",
            ]
        )

        self.assertIn("<th>Lint</th>", result)
        self.assertIn("<code>first</code>", result)
        self.assertIn("<strong>Useful</strong>", result)
        self.assertEqual(result.count('<td class="fix-yes">✓</td>'), 1)
        self.assertIn("<td>second</td>", result)

    def test_render_lint_table_rejects_missing_table(self) -> None:
        with self.assertRaisesRegex(SystemExit, "expected a Lint/Description"):
            page_builder.render_lint_table(["No table here"])

    def test_render_lint_table_accepts_a_row_without_an_auto_fix_column(self) -> None:
        result = page_builder.render_lint_table(
            [
                "| Lint | Description |",
                "| --- | --- |",
                "| safety | Still linted |",
            ]
        )

        self.assertIn("<td>safety</td><td>Still linted</td><td></td>", result.replace("\n", ""))

    def test_first_code_block_returns_contents(self) -> None:
        self.assertEqual(
            page_builder.first_code_block(
                ["prose", "```console", "command --flag", "second line", "```"]
            ),
            "command --flag\nsecond line",
        )

    def test_first_code_block_rejects_missing_or_unterminated_fence(self) -> None:
        with self.assertRaisesRegex(SystemExit, "fenced code block"):
            page_builder.first_code_block(["prose"])
        with self.assertRaisesRegex(SystemExit, "unterminated"):
            page_builder.first_code_block(["```console", "command"])

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

    def test_resolve_doc_href_handles_docs_repository_and_external_links(self) -> None:
        with (
            patch.object(page_builder, "DOC_FILENAME_TO_SLUG", {"guide.md": "guide"}),
            patch.object(page_builder, "GITHUB_BLOB_BASE", "https://example.test/repository"),
        ):
            self.assertEqual(page_builder.resolve_doc_href("guide.md"), "guide.html")
            self.assertEqual(
                page_builder.resolve_doc_href("../rules/example.yaml"),
                "https://example.test/repository/rules/example.yaml",
            )
            self.assertEqual(page_builder.resolve_doc_href("https://example.com"), "https://example.com")

    def test_strip_markdown_inline_produces_plain_text(self) -> None:
        self.assertEqual(
            page_builder.strip_markdown_inline(
                "Read **the [`configuration`](config.html)** for `details`."
            ),
            "Read the configuration for details.",
        )

    def test_first_paragraph_skips_headings_and_joins_wrapped_lines(self) -> None:
        lines = ["# Title", "", "First line with `code`", "continues here.", "", "Second paragraph."]

        self.assertEqual(
            page_builder.first_paragraph(lines),
            "First line with `code` continues here.",
        )

    def test_markdown_to_html_renders_headings_paragraphs_and_code(self) -> None:
        result = page_builder.markdown_to_html(
            [
                "## Setup **now**",
                "Read [the guide](guide.md)",
                "on the next line.",
                "",
                "```yaml",
                "unsafe: <value>",
                "```",
            ],
            lambda href: f"docs/{href}",
        )

        self.assertIn("<h2>Setup <strong>now</strong></h2>", result)
        self.assertIn('<p>Read <a href="docs/guide.md">the guide</a> on the next line.</p>', result)
        self.assertIn(
            '<pre class="code-block"><code>unsafe: &lt;value&gt;</code></pre>',
            result,
        )

    def test_markdown_to_html_flushes_a_final_paragraph(self) -> None:
        result = page_builder.markdown_to_html(["A paragraph", "continued without a blank line."])

        self.assertEqual(result, "<p>A paragraph continued without a blank line.</p>")

    def test_markdown_to_html_accepts_an_unclosed_final_code_fence(self) -> None:
        result = page_builder.markdown_to_html(["```text", "first", "  second"])

        self.assertEqual(
            result,
            '<pre class="code-block"><code>first\n  second</code></pre>',
        )

    def test_first_paragraph_returns_empty_text_when_there_is_no_prose(self) -> None:
        self.assertEqual(page_builder.first_paragraph(["# Title", "", "## Subtitle"]), "")

    def test_first_paragraph_skips_fenced_code_before_prose(self) -> None:
        lines = [
            "# Guide",
            "```yaml",
            "setting: value",
            "```",
            "",
            "The actual introduction.",
        ]

        self.assertEqual(page_builder.first_paragraph(lines), "The actual introduction.")

    def test_first_paragraph_ignores_an_unclosed_fenced_code_block(self) -> None:
        self.assertEqual(page_builder.first_paragraph(["```text", "not prose"]), "")


class DocsRenderingTest(unittest.TestCase):
    def test_raw_github_link_escapes_a_custom_source_url(self) -> None:
        result = page_builder.raw_github_link(
            {
                "filename": "guide.md",
                "source_url": 'https://example.test/source?label="docs"&mode=raw',
            }
        )

        self.assertIn(
            'href="https://example.test/source?label=&quot;docs&quot;&amp;mode=raw"', result
        )
        self.assertNotIn('label="docs"', result)

    def test_render_doc_renders_markdown_metadata_links_and_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "guide.md").write_text(
                "# Guide\n\nRead [`other`](other.md) before starting.\n",
                encoding="utf-8",
            )
            doc = {
                "filename": "guide.md",
                "slug": "guide",
                "kind": "markdown",
                "source_url": "https://example.test/source",
            }

            with (
                patch.object(page_builder, "DOCS_DIR", docs_dir),
                patch.object(page_builder, "DOC_FILENAME_TO_SLUG", {"other.md": "other"}),
            ):
                title, description, content = page_builder.render_doc(doc)

        self.assertEqual(title, "Guide")
        self.assertEqual(description, "Read other before starting.")
        self.assertIn('<a href="other.html"><code>other</code></a>', content)
        self.assertIn('href="https://example.test/source"', content)

    def test_render_doc_uses_filename_when_markdown_has_no_title(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "notes.md").write_text("Opening paragraph.\n", encoding="utf-8")
            doc = {"filename": "notes.md", "slug": "notes", "kind": "markdown"}

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                title, description, _ = page_builder.render_doc(doc)

        self.assertEqual(title, "notes.md")
        self.assertEqual(description, "Opening paragraph.")

    def test_render_doc_uses_default_repository_source_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "notes.md").write_text("# Notes\n", encoding="utf-8")

            with (
                patch.object(page_builder, "DOCS_DIR", docs_dir),
                patch.object(page_builder, "GITHUB_BLOB_BASE", "https://example.test/repo"),
            ):
                _, description, content = page_builder.render_doc(
                    {"filename": "notes.md", "slug": "notes", "kind": "markdown"}
                )

        self.assertEqual(description, "")
        self.assertIn('href="https://example.test/repo/docs/notes.md"', content)

    def test_render_doc_renders_json_schema_and_plain_text(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "schema.json").write_text(
                '{"title":"Report <schema>","description":"A & B","type":"object"}',
                encoding="utf-8",
            )
            (docs_dir / "config.yaml").write_text("setting: <value>\n", encoding="utf-8")

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                schema = page_builder.render_doc(
                    {"filename": "schema.json", "slug": "schema", "kind": "json-schema"}
                )
                plain = page_builder.render_doc(
                    {
                        "filename": "config.yaml",
                        "slug": "config",
                        "kind": "yaml",
                        "title": "Configuration",
                        "description": "All settings",
                    }
                )

        self.assertEqual(schema[:2], ("Report <schema>", "A & B"))
        self.assertIn("Report &lt;schema&gt;", schema[2])
        self.assertEqual(plain[:2], ("Configuration", "All settings"))
        self.assertIn("setting: &lt;value&gt;", plain[2])

    def test_render_doc_uses_filename_defaults_for_schema_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "schema.json").write_text('{"type":"string"}', encoding="utf-8")

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                title, description, content = page_builder.render_doc(
                    {"filename": "schema.json", "slug": "schema", "kind": "json-schema"}
                )

        self.assertEqual(title, "schema.json")
        self.assertEqual(description, "")
        self.assertIn('&quot;type&quot;: &quot;string&quot;', content)

    def test_render_docs_list_items_escapes_content_and_applies_prefix(self) -> None:
        docs = [{"slug": "guide", "blurb": "Use <carefully> & safely"}]
        results = {"guide": {"title": "Guide & reference"}}

        with patch.object(page_builder, "DOCS", docs):
            output = page_builder.render_docs_list_items(results, "docs/")

        self.assertIn('href="docs/guide.html"', output)
        self.assertIn("Guide &amp; reference", output)
        self.assertIn("Use &lt;carefully&gt; &amp; safely", output)

    def test_build_doc_pages_writes_detail_and_index_pages(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "docs.template.html").write_text(
                "<title><!--DOC_TITLE--></title>"
                '<meta content="<!--DOC_DESCRIPTION-->">'
                '<link href="<!--DOC_URL-->">'
                "<main><!--DOC_CONTENT--></main>",
                encoding="utf-8",
            )
            docs = [{"slug": "guide", "blurb": "A useful guide"}]
            results = {
                "guide": {
                    "title": "Guide & help",
                    "description": 'Use "care" & attention',
                    "content_html": "<p>Contents</p>",
                }
            }

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "DOCS", docs),
                patch.object(page_builder, "SITE_URL", "https://example.test/"),
            ):
                page_builder.build_doc_pages(out_dir, results)

            detail = (out_dir / "docs" / "guide.html").read_text(encoding="utf-8")
            index = (out_dir / "docs" / "index.html").read_text(encoding="utf-8")

        self.assertIn("<title>Guide &amp; help</title>", detail)
        self.assertIn('content="Use &quot;care&quot; &amp; attention"', detail)
        self.assertIn('href="https://example.test/docs/guide.html"', detail)
        self.assertIn("<p>Contents</p>", detail)
        self.assertIn('href="guide.html"', index)
        self.assertIn("A useful guide", index)


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
            ):
                with self.assertRaisesRegex(SystemExit, "missing marker"):
                    page_builder.build_videos_page(out_dir)

            self.assertFalse((out_dir / "videos.html").exists())


class MinifyTest(unittest.TestCase):
    def test_minify_html_strips_comments_and_collapses_indentation(self) -> None:
        source = """<main>
          <!-- a comment -->
          <p>
            Hello
          </p>


          <p>World</p>
        </main>"""

        result = page_builder.minify_html(source)

        self.assertNotIn("<!--", result)
        self.assertNotIn("  ", result)
        self.assertNotIn("\n", result)
        self.assertIn("<p> Hello </p>", result)
        self.assertIn("<p>World</p>", result)

    def test_minify_html_preserves_pre_block_whitespace_verbatim(self) -> None:
        pre_block = '<pre class="code-block"><code>line one\n    indented line\n\n\nline four</code></pre>'
        source = f"<main>\n  <p>  before  </p>\n  {pre_block}\n  <p>after</p>\n</main>"

        result = page_builder.minify_html(source)

        self.assertIn(pre_block, result)

    def test_minify_html_preserves_multiple_pre_blocks_in_order(self) -> None:
        first = "<pre>first\n  indented</pre>"
        second = "<pre><code>second\n\nlast</code></pre>"

        result = page_builder.minify_html(f"<main>\n{first}\n<p>middle</p>\n{second}\n</main>")

        self.assertIn(first, result)
        self.assertIn(second, result)
        self.assertLess(result.index(first), result.index(second))

    def test_minify_css_strips_comments_and_collapses_whitespace(self) -> None:
        source = """/* header */
        main {
          color: red;
          margin: 0 ;
        }

        .a, .b {
          display: flex;
        }
        """

        result = page_builder.minify_css(source)

        self.assertEqual(result, "main{color:red;margin:0}.a,.b{display:flex}")

    def test_finalize_page_wraps_images_before_removing_template_comments(self) -> None:
        source = """<!-- generated -->
        <main>
          <img src="assets/logo-small.jpg" alt="Logo" />
        </main>"""

        with patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"logo-small.jpg"}):
            result = page_builder.finalize_page(source)

        self.assertNotIn("<!--", result)
        self.assertNotIn("\n", result)
        self.assertIn('<source srcset="assets/logo-small.avif" type="image/avif" />', result)
        self.assertIn('<source srcset="assets/logo-small.webp" type="image/webp" />', result)


class ModernImageFormatsTest(unittest.TestCase):
    def test_convert_to_modern_formats_writes_webp_and_avif_siblings(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            source = out_dir / "screenshot.png"
            Image.new("RGBA", (4, 4), (10, 20, 30, 255)).save(source)

            page_builder.convert_to_modern_formats(source, out_dir)

            self.assertTrue((out_dir / "screenshot.webp").exists())
            self.assertTrue((out_dir / "screenshot.avif").exists())
            with Image.open(out_dir / "screenshot.webp") as webp_image:
                self.assertEqual(webp_image.size, (4, 4))
            with Image.open(out_dir / "screenshot.avif") as avif_image:
                self.assertEqual(avif_image.size, (4, 4))

    def test_convert_to_modern_formats_converts_non_rgb_jpeg_before_saving(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            out_dir = Path(directory)
            source = out_dir / "logo.jpg"
            Image.new("L", (3, 2), 128).save(source)

            page_builder.convert_to_modern_formats(source, out_dir)

            with Image.open(out_dir / "logo.webp") as webp_image:
                self.assertEqual((webp_image.mode, webp_image.size), ("RGB", (3, 2)))
            with Image.open(out_dir / "logo.avif") as avif_image:
                self.assertEqual((avif_image.mode, avif_image.size), ("RGB", (3, 2)))

    def test_wrap_images_with_modern_sources_wraps_only_known_assets(self) -> None:
        with patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"logo-small.jpg"}):
            page_html = (
                '<img class="site-header__logo" src="assets/logo-small.jpg" alt="" />'
                '<img src="assets/untouched.png" alt="not converted" />'
                '<img src="https://img.shields.io/badge/x" alt="badge" />'
            )

            result = page_builder.wrap_images_with_modern_sources(page_html)

        self.assertIn(
            '<picture><source srcset="assets/logo-small.avif" type="image/avif" />'
            '<source srcset="assets/logo-small.webp" type="image/webp" />'
            '<img class="site-header__logo" src="assets/logo-small.jpg" alt="" />'
            "</picture>",
            result,
        )
        self.assertIn('<img src="assets/untouched.png" alt="not converted" />', result)
        self.assertIn('<img src="https://img.shields.io/badge/x" alt="badge" />', result)
        self.assertEqual(result.count("<picture>"), 1)

    def test_wrap_images_with_modern_sources_respects_a_relative_docs_prefix(self) -> None:
        with patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"logo-small.jpg"}):
            result = page_builder.wrap_images_with_modern_sources(
                '<img class="site-header__logo" src="../assets/logo-small.jpg" alt="" />'
            )

        self.assertIn('<source srcset="../assets/logo-small.avif" type="image/avif" />', result)
        self.assertIn('<source srcset="../assets/logo-small.webp" type="image/webp" />', result)

    def test_wrap_images_with_modern_sources_preserves_query_like_non_asset_paths(self) -> None:
        page_html = '<img src="assets/logo-small.jpg?cache=1" alt="Logo" />'

        with patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"logo-small.jpg"}):
            result = page_builder.wrap_images_with_modern_sources(page_html)

        self.assertEqual(result, page_html)


class BuildTest(unittest.TestCase):
    def test_build_replaces_content_copies_assets_and_cleans_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            (root / "README.md").write_text(
                """## Implemented Lints

### Formatting
| Lint | Description | Auto-Fix |
| --- | --- | --- |
| `spacing` | Fix spacing | Yes |

## Command-line interface
```console
PapyrusLinterCLI example.psc
```
""",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<main><!--LINT_TABLE:Formatting--><!--CLI_EXAMPLES--><!--DOCS_LIST--><!--VERSION-->"
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
            (pages_dir / "styles.css").write_text("main { color: red; }", encoding="utf-8")
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

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "LINT_CATEGORIES", ["Formatting"]),
                patch.object(page_builder, "DOCS", []),
                patch.object(
                    page_builder,
                    "ASSETS",
                    {"copied.png": copied_asset, "screenshot.png": screenshot_asset},
                ),
                patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"screenshot.png"}),
            ):
                page_builder.build(out_dir, version="v1.2.3")

            output = (out_dir / "index.html").read_text(encoding="utf-8")
            self.assertIn("<code>spacing</code>", output)
            self.assertIn("PapyrusLinterCLI example.psc", output)
            self.assertIn("v1.2.3", output)
            self.assertNotIn("<!--LINT_TABLE", output)
            self.assertNotIn("<!--CLI_EXAMPLES-->", output)
            self.assertNotIn("<!--DOCS_LIST-->", output)
            self.assertNotIn("<!--VERSION-->", output)
            self.assertIn(
                '<picture><source srcset="assets/screenshot.avif" type="image/avif" />'
                '<source srcset="assets/screenshot.webp" type="image/webp" />'
                '<img src="assets/screenshot.png" alt="Screenshot" /></picture>',
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
            self.assertFalse((out_dir / "stale.txt").exists())
            self.assertTrue((out_dir / "docs" / "index.html").exists())

            videos_output = (out_dir / "videos.html").read_text(encoding="utf-8")
            self.assertIn("youtube-nocookie.com/embed/", videos_output)
            self.assertNotIn("<!--VIDEOS_LIST-->", videos_output)

    def test_build_rejects_a_missing_lint_table_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            (root / "README.md").write_text(
                """## Implemented Lints
### Formatting
| Lint | Description | Auto-Fix |
| --- | --- | --- |
| lint | description | |
## Command-line interface
```
command
```
""",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<!--CLI_EXAMPLES-->", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "LINT_CATEGORIES", ["Formatting"]),
            ):
                with self.assertRaisesRegex(SystemExit, "missing marker"):
                    page_builder.build(root / "out")

    def test_build_rejects_a_missing_cli_examples_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            (root / "README.md").write_text(
                """## Implemented Lints
### Formatting
| Lint | Description | Auto-Fix |
| --- | --- | --- |
| lint | description | |
## Command-line interface
```
command
```
""",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<!--LINT_TABLE:Formatting--><!--DOCS_LIST-->", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "LINT_CATEGORIES", ["Formatting"]),
                patch.object(page_builder, "DOCS", []),
            ):
                with self.assertRaisesRegex(SystemExit, "missing marker <!--CLI_EXAMPLES-->"):
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

        build.assert_called_once_with(output_dir, "")
        self.assertEqual(stdout.getvalue(), f"Built site into {output_dir}\n")

    def test_main_passes_an_explicit_version_to_build(self) -> None:
        output_dir = Path("versioned-output")

        with (
            patch("sys.argv", ["build.py", "--out", str(output_dir), "--version", "v9.8.7"]),
            patch.object(page_builder, "build") as build,
            patch("sys.stdout", new_callable=StringIO),
        ):
            page_builder.main()

        build.assert_called_once_with(output_dir, "v9.8.7")


if __name__ == "__main__":
    unittest.main()
