"""Tests for the dependency-free GitHub Pages site builder."""

from __future__ import annotations

import json
import runpy
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import MagicMock, patch
from urllib.error import HTTPError, URLError

from PIL import Image

from pages import build as page_builder


class PublishedSchemaTest(unittest.TestCase):
    def test_ai_export_rule_details_expose_auto_fixability(self) -> None:
        schema = json.loads(
            (page_builder.DOCS_DIR / "papyrus-lint-ai-export.v3.schema.json").read_text(encoding="utf-8")
        )

        rule_detail = schema["$defs"]["ruleDetail"]
        self.assertIn("auto_fixable", rule_detail["required"])
        self.assertEqual(rule_detail["properties"]["auto_fixable"]["type"], "boolean")

    def test_ai_export_external_diagnostic_fields_require_each_other(self) -> None:
        schema = json.loads(
            (page_builder.DOCS_DIR / "papyrus-lint-ai-export.v3.schema.json").read_text(encoding="utf-8")
        )

        self.assertEqual(
            schema["$defs"]["diagnostic"]["dependentRequired"],
            {"external": ["source"], "source": ["external"]},
        )

    def test_ai_export_space_indentation_requires_positive_width(self) -> None:
        schema = json.loads(
            (page_builder.DOCS_DIR / "papyrus-lint-ai-export.v3.schema.json").read_text(encoding="utf-8")
        )

        configuration = schema["$defs"]["configuration"]
        self.assertEqual(
            configuration["if"],
            {"properties": {"indentation": {"const": "space"}}},
        )
        self.assertEqual(
            configuration["then"],
            {"properties": {"indentation_width": {"minimum": 1}}},
        )


class PageHelpersTest(unittest.TestCase):
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

    def test_render_shared_components_uses_one_source_with_page_relative_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            includes_dir = Path(directory)
            (includes_dir / "header.html").write_text(
                '<header><a href="<!--ROOT_PATH-->index.html">Home</a></header>', encoding="utf-8"
            )
            (includes_dir / "footer.html").write_text(
                "<footer><!--VERSION--></footer>", encoding="utf-8"
            )
            with patch.object(page_builder, "INCLUDES_DIR", includes_dir):
                result = page_builder.render_shared_components(
                    "<!--SITE_HEADER--><main>Docs</main><!--SITE_FOOTER-->", "../", "v1.2&3"
                )

        self.assertEqual(
            result,
            '<header><a href="../index.html">Home</a></header>'
            "<main>Docs</main><footer>v1.2&amp;3</footer>",
        )

    def test_render_shared_components_rejects_a_partially_shared_shell(self) -> None:
        with self.assertRaisesRegex(SystemExit, "missing shared component marker <!--SITE_FOOTER-->"):
            page_builder.render_shared_components("<!--SITE_HEADER--><main></main>", "", "")

    def test_render_shared_components_rejects_a_footer_without_a_header(self) -> None:
        with self.assertRaisesRegex(SystemExit, "missing shared component marker <!--SITE_HEADER-->"):
            page_builder.render_shared_components("<main></main><!--SITE_FOOTER-->", "", "")

    def test_render_shared_components_supports_fragment_only_templates(self) -> None:
        result = page_builder.render_shared_components(
            "<title><!--VERSION--></title><a href=\"<!--SITE_URL-->\">Site</a>",
            "../",
            "",
        )

        self.assertEqual(
            result,
            f'<title>unreleased</title><a href="{page_builder.SITE_URL}">Site</a>',
        )

    def test_render_shared_components_replaces_placeholders_inside_includes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            includes_dir = Path(directory)
            (includes_dir / "header.html").write_text(
                '<a href="<!--SITE_URL-->"><!--ROOT_PATH--></a>', encoding="utf-8"
            )
            (includes_dir / "footer.html").write_text(
                "<footer><!--FUNDING_LINKS--><!--VERSION--></footer>", encoding="utf-8"
            )
            with (
                patch.object(page_builder, "INCLUDES_DIR", includes_dir),
                patch.object(page_builder, "SITE_URL", "https://example.test/"),
                patch.object(page_builder, "render_funding_links", return_value="<li>Support</li>"),
            ):
                result = page_builder.render_shared_components(
                    "<!--SITE_HEADER--><!--SITE_FOOTER-->", "../", 'v1<&"'
                )

        self.assertEqual(
            result,
            '<a href="https://example.test/">../</a>'
            "<footer><li>Support</li>v1&lt;&amp;&quot;</footer>",
        )

    def test_parse_funding_values_accepts_scalars_lists_quotes_and_empty_values(self) -> None:
        self.assertEqual(page_builder.parse_funding_values(" sponsor "), ["sponsor"])
        self.assertEqual(
            page_builder.parse_funding_values("['first sponsor', \"second\", '']"),
            ["first sponsor", "second"],
        )
        self.assertEqual(page_builder.parse_funding_values("   "), [])

    def test_parse_funding_values_ignores_empty_inline_list_entries(self) -> None:
        self.assertEqual(
            page_builder.parse_funding_values("[first, , '', \"second account\"]"),
            ["first", "second account"],
        )

    def test_render_funding_links_reads_provider_and_custom_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text(
                "github: [first sponsor, second]\ncustom: https://www.paypal.com/donate?id=1&campaign=two\n",
                encoding="utf-8",
            )

            result = page_builder.render_funding_links(funding_file)

        self.assertIn('href="https://github.com/sponsors/first%20sponsor"', result)
        self.assertIn('href="https://github.com/sponsors/second"', result)
        self.assertIn(">GitHub Sponsors</a>", result)
        self.assertIn('href="https://www.paypal.com/donate?id=1&amp;campaign=two"', result)
        self.assertIn(">PayPal</a>", result)

    def test_render_funding_links_rejects_an_invalid_custom_url(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text("custom: javascript:alert(1)\n", encoding="utf-8")

            with self.assertRaisesRegex(SystemExit, "must be an HTTP\\(S\\) URL"):
                page_builder.render_funding_links(funding_file)

    def test_render_funding_links_rejects_custom_urls_without_a_web_host(self) -> None:
        invalid_urls = ("https:///missing-host", "mailto:maintainer@example.test")

        for invalid_url in invalid_urls:
            with self.subTest(url=invalid_url), tempfile.TemporaryDirectory() as directory:
                funding_file = Path(directory) / "FUNDING.yml"
                funding_file.write_text(f"custom: {invalid_url}\n", encoding="utf-8")

                with self.assertRaisesRegex(
                    SystemExit,
                    f"custom funding link must be an HTTP\\(S\\) URL: {invalid_url}",
                ):
                    page_builder.render_funding_links(funding_file)

    def test_render_funding_links_skips_comments_malformed_lines_and_unknown_providers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text(
                "# Maintainer funding\nmalformed line\nunknown: account\ngithub: valid-user\n",
                encoding="utf-8",
            )

            result = page_builder.render_funding_links(funding_file)

        self.assertEqual(result.count("<li>"), 1)
        self.assertIn("https://github.com/sponsors/valid-user", result)
        self.assertNotIn("unknown", result)

    def test_render_funding_links_supports_each_named_provider(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text(
                "\n".join(f"{provider}: account/name" for provider in page_builder.FUNDING_PROVIDERS),
                encoding="utf-8",
            )

            result = page_builder.render_funding_links(funding_file)

        self.assertEqual(result.count("<li>"), len(page_builder.FUNDING_PROVIDERS))
        for label, url_template in page_builder.FUNDING_PROVIDERS.values():
            self.assertIn(f">{label}</a>", result)
            self.assertIn(url_template.format("account%2Fname"), result)

    def test_render_funding_links_labels_non_paypal_custom_urls_generically(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text("custom: https://example.test/support\n", encoding="utf-8")

            result = page_builder.render_funding_links(funding_file)

        self.assertIn(">Support this project</a>", result)
        self.assertIn('href="https://example.test/support"', result)

    def test_render_funding_links_encodes_provider_handles_as_path_segments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text("github: user/name\n", encoding="utf-8")

            result = page_builder.render_funding_links(funding_file)

        self.assertIn("https://github.com/sponsors/user%2Fname", result)
        self.assertNotIn("sponsors/user/name", result)

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

    def test_resolve_doc_href_only_rewrites_a_leading_parent_segment(self) -> None:
        with patch.object(page_builder, "GITHUB_BLOB_BASE", "https://example.test/repository"):
            self.assertEqual(
                page_builder.resolve_doc_href("../docs/guide.md#setup"),
                "https://example.test/repository/docs/guide.md#setup",
            )
            self.assertEqual(page_builder.resolve_doc_href("guide/../notes.md"), "guide/../notes.md")

class DocsRenderingTest(unittest.TestCase):
    def test_load_doc_source_downloads_remote_documentation(self) -> None:
        response = MagicMock()
        response.__enter__.return_value.read.return_value = b"# Current remote README\n"

        with patch.object(page_builder, "urlopen", return_value=response) as urlopen:
            source = page_builder.load_doc_source(
                {"content_url": "https://example.test/README.md"}
            )

        self.assertEqual(source, "# Current remote README\n")
        request = urlopen.call_args.args[0]
        self.assertEqual(request.full_url, "https://example.test/README.md")
        self.assertEqual(request.get_header("User-agent"), "papyrus-lint-pages-builder")
        self.assertEqual(urlopen.call_args.kwargs, {"timeout": 30})

    def test_load_doc_source_reports_remote_download_failure(self) -> None:
        with (
            patch.object(page_builder, "urlopen", side_effect=URLError("offline")),
            self.assertRaisesRegex(
                SystemExit,
                "Could not download documentation from https://example.test/README.md",
            ),
        ):
            page_builder.load_doc_source(
                {"content_url": "https://example.test/README.md"}
            )

    def test_load_doc_source_reports_an_http_error_with_the_source_url(self) -> None:
        error = HTTPError(
            "https://example.test/missing.md",
            404,
            "Not Found",
            hdrs=None,
            fp=None,
        )

        with (
            patch.object(page_builder, "urlopen", side_effect=error),
            self.assertRaisesRegex(
                SystemExit,
                "Could not download documentation from https://example.test/missing.md: HTTP Error 404",
            ),
        ):
            page_builder.load_doc_source(
                {"content_url": "https://example.test/missing.md"}
            )

    def test_load_doc_source_reports_remote_timeout(self) -> None:
        with (
            patch.object(page_builder, "urlopen", side_effect=TimeoutError("timed out")),
            self.assertRaisesRegex(
                SystemExit,
                "Could not download documentation from https://example.test/README.md: timed out",
            ),
        ):
            page_builder.load_doc_source(
                {"content_url": "https://example.test/README.md"}
            )

    def test_load_doc_source_reports_invalid_remote_utf8(self) -> None:
        response = MagicMock()
        response.__enter__.return_value.read.return_value = b"\xff"

        with (
            patch.object(page_builder, "urlopen", return_value=response),
            self.assertRaisesRegex(SystemExit, "Could not download documentation"),
        ):
            page_builder.load_doc_source({"content_url": "https://example.test/README.md"})

    def test_load_doc_source_reads_local_documentation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "guide.md").write_text("# Local guide\n", encoding="utf-8")

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                source = page_builder.load_doc_source({"filename": "guide.md"})

        self.assertEqual(source, "# Local guide\n")

    def test_load_doc_source_propagates_a_missing_local_document(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(page_builder, "DOCS_DIR", Path(directory)),
            self.assertRaises(FileNotFoundError),
        ):
            page_builder.load_doc_source({"filename": "missing.md"})

    def test_raw_github_link_escapes_a_custom_source_url(self) -> None:
        result = page_builder.raw_github_link(
            {
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

    def test_render_doc_keeps_a_non_title_heading_in_the_markdown_body(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "notes.md").write_text(
                "## Overview\n\nOpening paragraph.\n", encoding="utf-8"
            )

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                title, description, content = page_builder.render_doc(
                    {"filename": "notes.md", "slug": "notes", "kind": "markdown"}
                )

        self.assertEqual(title, "notes.md")
        self.assertEqual(description, "Opening paragraph.")
        self.assertIn("<h2>Overview</h2>", content)

    def test_render_doc_handles_an_empty_markdown_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "empty.md").write_text("", encoding="utf-8")

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                title, description, content = page_builder.render_doc(
                    {"filename": "empty.md", "slug": "empty", "kind": "markdown"}
                )

        self.assertEqual(title, "empty.md")
        self.assertEqual(description, "")
        self.assertIn("View raw source on GitHub", content)

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

    def test_render_doc_escapes_plain_text_and_appends_the_raw_source_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "example.bbcode").write_text(
                '[url="javascript:alert(1)"]<unsafe> & text[/url]', encoding="utf-8"
            )

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                title, description, content = page_builder.render_doc(
                    {
                        "filename": "example.bbcode",
                        "slug": "example",
                        "kind": "bbcode",
                        "title": "Example source",
                        "description": "A safe preview",
                    }
                )

        self.assertEqual((title, description), ("Example source", "A safe preview"))
        self.assertIn("&lt;unsafe&gt; &amp; text", content)
        self.assertNotIn("<unsafe>", content)
        self.assertIn("View raw source on GitHub", content)

    def test_render_doc_highlights_yaml_and_bbcode_sources(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "config.yaml").write_text(
                'enabled: true\nlabel: "stable" # documented\n', encoding="utf-8"
            )
            (docs_dir / "listing.bbcode").write_text(
                "[b]Important[/b]", encoding="utf-8"
            )

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                yaml_doc = page_builder.render_doc(
                    {
                        "filename": "config.yaml",
                        "kind": "yaml",
                        "title": "Configuration",
                        "description": "Settings",
                    }
                )
                bbcode_doc = page_builder.render_doc(
                    {
                        "filename": "listing.bbcode",
                        "kind": "bbcode",
                        "title": "Listing",
                        "description": "Source",
                    }
                )

        self.assertIn('<span class="kw">true</span>', yaml_doc[2])
        self.assertIn('<span class="str">&quot;stable&quot;</span>', yaml_doc[2])
        self.assertIn('<span class="cm"># documented</span>', yaml_doc[2])
        self.assertIn('<span class="tag">[b]</span>', bbcode_doc[2])
        self.assertIn('<span class="tag">[/b]</span>', bbcode_doc[2])

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
        self.assertIn('<span class="str">&quot;type&quot;</span>', content)
        self.assertIn('<span class="str">&quot;string&quot;</span>', content)

    def test_render_doc_prefers_a_short_configured_schema_description(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "schema.json").write_text(
                '{"title":"Schema","description":"A very long schema description."}',
                encoding="utf-8",
            )
            doc = {
                "filename": "schema.json",
                "slug": "schema",
                "kind": "json-schema",
                "description": "Short page summary.",
            }

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                title, description, content = page_builder.render_doc(doc)

        self.assertEqual((title, description), ("Schema", "Short page summary."))
        self.assertIn("A very long schema description.", content)

    def test_render_doc_propagates_invalid_json_schema_input(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "schema.json").write_text("{not valid json", encoding="utf-8")

            with (
                patch.object(page_builder, "DOCS_DIR", docs_dir),
                self.assertRaisesRegex(ValueError, "Expecting property name"),
            ):
                page_builder.render_doc(
                    {"filename": "schema.json", "slug": "schema", "kind": "json-schema"}
                )

    def test_render_docs_list_items_escapes_content_and_applies_prefix(self) -> None:
        docs = [{"slug": "guide", "blurb": "Use <carefully> & safely"}]
        results = {"guide": {"title": "Guide & reference"}}

        with patch.object(page_builder, "DOCS", docs):
            output = page_builder.render_docs_list_items(results, "docs/")

        self.assertIn('href="docs/guide.html"', output)
        self.assertIn("Guide &amp; reference", output)
        self.assertIn("Use &lt;carefully&gt; &amp; safely", output)

    def test_render_docs_list_items_preserves_configured_document_order(self) -> None:
        docs = [
            {"slug": "second", "blurb": "Second blurb"},
            {"slug": "first", "blurb": "First blurb"},
        ]
        results = {
            "first": {"title": "First"},
            "second": {"title": "Second"},
        }

        with patch.object(page_builder, "DOCS", docs):
            output = page_builder.render_docs_list_items(results, "")

        self.assertLess(output.index("second.html"), output.index("first.html"))
        self.assertEqual(output.count("<li>"), 2)

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
                patch.object(page_builder, "INCLUDES_DIR", includes_dir),
                patch.object(page_builder, "render_funding_links", return_value=""),
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
                patch.object(page_builder, "INCLUDES_DIR", includes_dir),
                patch.object(page_builder, "render_funding_links", return_value=""),
            ):
                page_builder.build_imprint_page(out_dir, 'v2<&"')

            output = (out_dir / "imprint.html").read_text(encoding="utf-8")

        self.assertIn("<header>Imprint</header>", output)
        self.assertIn("Legal Notice", output)
        self.assertIn("<footer>v2&lt;&amp;&quot;</footer>", output)


class ActionPageTest(unittest.TestCase):
    def test_build_action_page_renders_the_downloaded_readme_and_replaces_markers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "action.template.html").write_text(
                "<title><!--ACTION_TITLE--></title>"
                '<meta content="<!--ACTION_DESCRIPTION-->">'
                "<main><!--ACTION_CONTENT--></main>",
                encoding="utf-8",
            )
            response = MagicMock()
            response.__enter__.return_value.read.return_value = (
                b"# Papyrus Lint Action\n\nLints pull requests automatically.\n"
            )

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "urlopen", return_value=response),
            ):
                page_builder.build_action_page(out_dir, version="v1.0.0")

            output = (out_dir / "action.html").read_text(encoding="utf-8")

        self.assertIn("<title>Papyrus Lint Action</title>", output)
        self.assertIn('content="Lints pull requests automatically."', output)
        self.assertIn("<p>Lints pull requests automatically.</p>", output)
        self.assertIn("View raw source on GitHub", output)
        self.assertNotIn("<!--ACTION_TITLE-->", output)
        self.assertNotIn("<!--ACTION_DESCRIPTION-->", output)
        self.assertNotIn("<!--ACTION_CONTENT-->", output)

    def test_build_action_page_rejects_a_template_missing_a_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "action.template.html").write_text(
                "<title><!--ACTION_TITLE--></title><main><!--ACTION_CONTENT--></main>",
                encoding="utf-8",
            )
            response = MagicMock()
            response.__enter__.return_value.read.return_value = b"# Title\n\nBody.\n"

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "urlopen", return_value=response),
                self.assertRaisesRegex(SystemExit, "missing marker <!--ACTION_DESCRIPTION-->"),
            ):
                page_builder.build_action_page(out_dir)

            self.assertFalse((out_dir / "action.html").exists())


class RulesPageTest(unittest.TestCase):
    def test_build_rules_page_renders_table_and_filters(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            rules_file = root / "rules.json"
            rules_file.write_text(
                json.dumps(
                    [
                        {
                            "id": "example-rule",
                            "name": "Example rule",
                            "tags": ["correctness", "style"],
                            "severity": "warning",
                            "fixable": True,
                            "description": "Flags an `example`.",
                            "definition": "The full behavior of this rule.",
                        }
                    ]
                ),
                encoding="utf-8",
            )
            (pages_dir / "rules.template.html").write_text(
                "<title>Lint rules</title><main><!--RULES_CONTENT--></main>", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "RULES_FILE", rules_file),
            ):
                page_builder.build_rules_page(out_dir, version="v1.0.0")

            output = (out_dir / "rules.html").read_text(encoding="utf-8")

        self.assertNotIn("<!--RULES_CONTENT-->", output)
        self.assertIn('id="rule-example-rule"', output)
        self.assertIn("Example rule", output)
        self.assertIn('<code>example-rule</code>', output)
        self.assertIn('data-severity="warning"', output)
        self.assertIn('data-tags="correctness style"', output)
        self.assertIn('data-fixable="true"', output)
        self.assertIn('<td class="fix-yes">✓</td>', output)
        self.assertIn("<code>example</code>", output)
        self.assertIn("The full behavior of this rule.", output)
        self.assertIn('value="warning"', output)
        self.assertIn('value="correctness"', output)
        self.assertIn('value="style"', output)
        self.assertNotIn('value="performance"', output)
        self.assertNotIn('value="maintainability"', output)
        self.assertNotIn('value="error"', output)
        self.assertNotIn('value="info"', output)
        self.assertIn('id="rules-fixable-filter"', output)
        self.assertIn('id="rules-count"', output)

    def test_build_rules_page_rejects_a_template_missing_a_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            rules_file = root / "rules.json"
            rules_file.write_text("[]", encoding="utf-8")
            (pages_dir / "rules.template.html").write_text("<main>No marker</main>", encoding="utf-8")

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "RULES_FILE", rules_file),
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
                page_builder.build_rules_page(out_dir)

            self.assertFalse((out_dir / "rules.html").exists())


class RepositoryConfigurationTest(unittest.TestCase):
    """Keep build.py's checked-in inputs synchronized with its manifest data."""

    def test_rules_json_has_unique_ids_and_known_severities_and_tags(self) -> None:
        rules = page_builder.load_rules()

        self.assertTrue(rules)
        ids = [rule["id"] for rule in rules]
        self.assertEqual(len(ids), len(set(ids)), "rule ids must be unique")
        for rule in rules:
            with self.subTest(rule=rule["id"]):
                self.assertIn(rule["severity"], page_builder.RULE_SEVERITIES)
                self.assertTrue(rule["tags"])
                for tag in rule["tags"]:
                    self.assertIn(tag, page_builder.RULE_TAGS)
                self.assertIsInstance(rule["fixable"], bool)
                self.assertTrue(rule["name"].strip())
                self.assertTrue(rule["description"].strip())
                self.assertTrue(rule["definition"].strip())

    def test_document_manifest_has_unique_slugs_and_readable_local_sources(self) -> None:
        slugs = [doc["slug"] for doc in page_builder.DOCS]
        filenames = [doc["filename"] for doc in page_builder.DOCS if "filename" in doc]

        self.assertEqual(len(slugs), len(set(slugs)), "documentation slugs must be unique")
        self.assertEqual(len(filenames), len(set(filenames)), "documentation sources must be unique")
        for filename in filenames:
            source = page_builder.DOCS_DIR / filename
            with self.subTest(filename=filename):
                self.assertTrue(source.is_file(), f"missing documentation source: {source}")
                self.assertTrue(source.read_text(encoding="utf-8").strip())

    def test_every_local_document_renders_with_metadata_and_a_source_link(self) -> None:
        for doc in page_builder.DOCS:
            if "content_url" in doc:
                continue
            with self.subTest(slug=doc["slug"]):
                title, description, content = page_builder.render_doc(doc)

                self.assertTrue(title.strip())
                self.assertTrue(description.strip())
                self.assertTrue(content.strip())
                self.assertIn("View raw source on GitHub", content)
                self.assertIn(f"/docs/{doc['filename']}", content)

    def test_copy_json_schemas_publishes_only_schemata_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            docs_dir = root / "docs"
            docs_dir.mkdir()
            (docs_dir / "first.schema.json").write_bytes(b'{"title": "First"}\n')
            (docs_dir / "second.schema.json").write_bytes(b'{\n  "type": "object"\n}\n')
            (docs_dir / page_builder.AI_EXPORT_V1_SCHEMA).write_bytes(b'{"title": "AI export v1"}\n')
            (docs_dir / "ordinary.json").write_bytes(b"{}\n")
            out_dir = root / "site"
            out_dir.mkdir()

            with patch.object(page_builder, "DOCS_DIR", docs_dir):
                page_builder.copy_json_schemas(out_dir)

            self.assertEqual(
                (out_dir / "schema" / "first.schema.json").read_bytes(),
                b'{"title": "First"}\n',
            )
            self.assertEqual(
                (out_dir / "schema" / "second.schema.json").read_bytes(),
                b'{\n  "type": "object"\n}\n',
            )
            self.assertEqual(
                (out_dir / "schema" / page_builder.AI_EXPORT_LEGACY_SCHEMA).read_bytes(),
                b'{"title": "AI export v1"}\n',
            )
            self.assertFalse((out_dir / "schema" / "ordinary.json").exists())

    def test_asset_manifest_points_to_files_and_modern_assets_are_a_subset(self) -> None:
        self.assertLessEqual(page_builder.MODERN_FORMAT_ASSETS, page_builder.ASSETS.keys())
        for output_name, source in page_builder.ASSETS.items():
            with self.subTest(asset=output_name):
                self.assertTrue(source.is_file(), f"missing site asset: {source}")
                self.assertEqual(Path(output_name).suffix.lower(), source.suffix.lower())

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
            "index.template.html": {"<!--CLI_EXAMPLES-->", "<!--DOCS_LIST-->"},
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
            with patch.object(page_builder, "urlopen", return_value=action_response):
                page_builder.build(out_dir, version="v9.8.7")

            expected_html = {
                "index.html",
                "action.html",
                "rules.html",
                "videos.html",
                "coverage.html",
                "imprint.html",
                "docs/index.html",
                *(f"docs/{doc['slug']}.html" for doc in page_builder.DOCS),
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
            self.assertIn('href="rules.html"', index)
            for doc in page_builder.DOCS:
                with self.subTest(homepage_doc=doc["slug"]):
                    self.assertIn(f'href="docs/{doc["slug"]}.html"', index)

            docs_index = (out_dir / "docs" / "index.html").read_text(encoding="utf-8")
            self.assertIn('href="../index.html#top"', docs_index)
            self.assertIn('src="../theme.js"', docs_index)
            self.assertIn('href="../styles.css"', docs_index)
            for doc in page_builder.DOCS:
                with self.subTest(docs_index_doc=doc["slug"]):
                    self.assertIn(f'href="{doc["slug"]}.html"', docs_index)

            expected_schemas = {
                path.name: path.read_bytes()
                for path in page_builder.DOCS_DIR.glob(page_builder.SCHEMA_GLOB)
            }
            expected_schemas[page_builder.AI_EXPORT_LEGACY_SCHEMA] = (
                page_builder.DOCS_DIR / page_builder.AI_EXPORT_V1_SCHEMA
            ).read_bytes()
            published_schemas = {
                path.name: path.read_bytes() for path in (out_dir / "schema").glob("*.json")
            }
            self.assertEqual(published_schemas, expected_schemas)

            self.assertEqual(
                (out_dir / "CNAME").read_text(encoding="utf-8"),
                page_builder.CNAME_FILE.read_text(encoding="utf-8"),
            )
            css = (out_dir / "styles.css").read_text(encoding="utf-8")
            self.assertNotIn("@import", css)
            self.assertIn("--color-accent:", css)
            self.assertIn("--font-display:", css)
            self.assertIn(".button--primary", css)
            self.assertIn("@font-face", css)
            for output_name in page_builder.ASSETS:
                with self.subTest(asset=output_name):
                    self.assertTrue((out_dir / "assets" / output_name).is_file())
            for output_name in page_builder.MODERN_FORMAT_ASSETS:
                stem = Path(output_name).stem
                with self.subTest(modern_asset=output_name):
                    self.assertTrue((out_dir / "assets" / f"{stem}.webp").is_file())
                    self.assertTrue((out_dir / "assets" / f"{stem}.avif").is_file())

            self.assertTrue((out_dir / "rules.js").is_file())
            rules = page_builder.load_rules()
            rules_output = (out_dir / "rules.html").read_text(encoding="utf-8")
            self.assertIn(str(len(rules)), rules_output)
            for rule in rules:
                with self.subTest(rule=rule["id"]):
                    self.assertIn(f'id="rule-{rule["id"]}"', rules_output)


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
            (root / "README.md").write_text(
                """## Command-line interface
```console
PapyrusLinterCLI example.psc
```
""",
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
                patch.object(
                    page_builder,
                    "ASSETS",
                    {"copied.png": copied_asset, "screenshot.png": screenshot_asset},
                ),
                patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"screenshot.png"}),
                patch.object(page_builder, "urlopen", return_value=action_response),
                patch.object(page_builder, "RULES_FILE", rules_file),
            ):
                page_builder.build(out_dir, version="v1.2.3")

            output = (out_dir / "index.html").read_text(encoding="utf-8")
            self.assertIn("PapyrusLinterCLI example.psc", output)
            self.assertIn("v1.2.3", output)
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
                page_builder.CNAME_FILE.read_text(encoding="utf-8"),
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
            (root / "README.md").write_text(
                """## Command-line interface
```
command
```
""",
                encoding="utf-8",
            )
            (pages_dir / "index.template.html").write_text(
                "<!--DOCS_LIST-->", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "DOCS", []),
                self.assertRaisesRegex(SystemExit, "missing marker <!--CLI_EXAMPLES-->"),
            ):
                page_builder.build(root / "out")

            self.assertFalse((root / "out").exists())

    def test_build_rejects_a_missing_docs_list_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            (root / "README.md").write_text(
                """## Command-line interface
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
                patch.object(page_builder, "DOCS", []),
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


class CoveragePageTest(unittest.TestCase):
    def test_build_coverage_page_renders_a_placeholder_without_a_coverage_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<title><!--COVERAGE_VERSION--></title><main><!--COVERAGE_CONTENT--></main>",
                encoding="utf-8",
            )

            with patch.object(page_builder, "PAGES_DIR", pages_dir):
                page_builder.build_coverage_page(out_dir, None, "")

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("unreleased", output)
        self.assertIn("Coverage data isn't available for this build.", output)

    def test_build_coverage_page_uses_placeholder_for_a_missing_coverage_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<main><!--COVERAGE_CONTENT--></main>", encoding="utf-8"
            )

            with patch.object(page_builder, "PAGES_DIR", pages_dir):
                page_builder.build_coverage_page(out_dir, root / "missing", "v2.0.0")

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("Coverage data isn't available for this build.", output)

    def test_build_coverage_page_renders_report_content_from_a_coverage_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            coverage_dir = root / "coverage-artifacts"
            pages_dir.mkdir()
            out_dir.mkdir()
            coverage_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<title><!--COVERAGE_VERSION--></title><main><!--COVERAGE_CONTENT--></main>",
                encoding="utf-8",
            )
            report_dir = coverage_dir / "rust-coverage-papyrus-parser"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text("SF:src/lib.rs\nLF:2\nLH:1\nend_of_record\n", encoding="utf-8")

            with patch.object(page_builder, "PAGES_DIR", pages_dir):
                page_builder.build_coverage_page(out_dir, coverage_dir, "v1.4.0")

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("v1.4.0", output)
        self.assertIn("papyrus-parser", output)
        self.assertIn("src/lib.rs", output)
        self.assertNotIn("Coverage data isn&#x27;t available", output)

    def test_build_coverage_page_escapes_the_version_label(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<title><!--COVERAGE_VERSION--></title><main><!--COVERAGE_CONTENT--></main>",
                encoding="utf-8",
            )

            with patch.object(page_builder, "PAGES_DIR", pages_dir):
                page_builder.build_coverage_page(out_dir, None, 'v1<&"')

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("<title>v1&lt;&amp;&quot;</title>", output)
        self.assertNotIn('v1<&"', output)

    def test_build_coverage_page_rejects_a_template_without_the_content_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text("<main>No marker</main>", encoding="utf-8")

            with (
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
                page_builder.build_coverage_page(root / "out", None, "")


if __name__ == "__main__":
    unittest.main()
