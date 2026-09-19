"""Tests for the docs/*.md subpages and the GitHub Action's own page."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch
from urllib.error import HTTPError, URLError

from pages import docs_pages


class PublishedSchemaTest(unittest.TestCase):
    def test_ai_export_rule_details_expose_auto_fixability(self) -> None:
        schema = json.loads(
            (docs_pages.SCHEMA_DIR / "papyrus-lint-ai-export.v3.schema.json").read_text(encoding="utf-8")
        )

        rule_detail = schema["$defs"]["ruleDetail"]
        self.assertIn("auto_fixable", rule_detail["required"])
        self.assertEqual(rule_detail["properties"]["auto_fixable"]["type"], "boolean")

    def test_ai_export_external_diagnostic_fields_require_each_other(self) -> None:
        schema = json.loads(
            (docs_pages.SCHEMA_DIR / "papyrus-lint-ai-export.v3.schema.json").read_text(encoding="utf-8")
        )

        self.assertEqual(
            schema["$defs"]["diagnostic"]["dependentRequired"],
            {"external": ["source"], "source": ["external"]},
        )

    def test_ai_export_space_indentation_requires_positive_width(self) -> None:
        schema = json.loads(
            (docs_pages.SCHEMA_DIR / "papyrus-lint-ai-export.v3.schema.json").read_text(encoding="utf-8")
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


class ResolveDocHrefTest(unittest.TestCase):
    def test_handles_docs_repository_and_external_links(self) -> None:
        with (
            patch.object(docs_pages, "DOC_FILENAME_TO_SLUG", {"guide.md": "guide"}),
            patch.object(docs_pages, "GITHUB_BLOB_BASE", "https://example.test/repository"),
        ):
            self.assertEqual(docs_pages.resolve_doc_href("guide.md"), "guide.html")
            self.assertEqual(
                docs_pages.resolve_doc_href("../shared/rules/data/example.yaml"),
                "https://example.test/repository/shared/rules/data/example.yaml",
            )
            self.assertEqual(docs_pages.resolve_doc_href("https://example.com"), "https://example.com")

    def test_only_rewrites_a_leading_parent_segment(self) -> None:
        with patch.object(docs_pages, "GITHUB_BLOB_BASE", "https://example.test/repository"):
            self.assertEqual(
                docs_pages.resolve_doc_href("../docs/guide.md#setup"),
                "https://example.test/repository/docs/guide.md#setup",
            )
            self.assertEqual(docs_pages.resolve_doc_href("guide/../notes.md"), "guide/../notes.md")

    def test_preserves_a_fragment_or_query_on_a_rewritten_doc_link(self) -> None:
        with patch.object(docs_pages, "DOC_FILENAME_TO_SLUG", {"guide.md": "guide"}):
            self.assertEqual(docs_pages.resolve_doc_href("guide.md#setup"), "guide.html#setup")
            self.assertEqual(docs_pages.resolve_doc_href("guide.md?view=full"), "guide.html?view=full")


class DocsRenderingTest(unittest.TestCase):
    def test_load_doc_source_downloads_remote_documentation(self) -> None:
        response = MagicMock()
        response.__enter__.return_value.read.return_value = b"# Current remote README\n"

        with patch.object(docs_pages, "urlopen", return_value=response) as urlopen:
            source = docs_pages.load_doc_source({"content_url": "https://example.test/README.md"})

        self.assertEqual(source, "# Current remote README\n")
        request = urlopen.call_args.args[0]
        self.assertEqual(request.full_url, "https://example.test/README.md")
        self.assertEqual(request.get_header("User-agent"), "papyrus-lint-pages-builder")
        self.assertEqual(urlopen.call_args.kwargs, {"timeout": 30})

    def test_load_doc_source_reports_remote_download_failure(self) -> None:
        with (
            patch.object(docs_pages, "urlopen", side_effect=URLError("offline")),
            self.assertRaisesRegex(
                SystemExit,
                "Could not download documentation from https://example.test/README.md",
            ),
        ):
            docs_pages.load_doc_source({"content_url": "https://example.test/README.md"})

    def test_load_doc_source_reports_an_http_error_with_the_source_url(self) -> None:
        error = HTTPError(
            "https://example.test/missing.md",
            404,
            "Not Found",
            hdrs=None,
            fp=None,
        )

        with (
            patch.object(docs_pages, "urlopen", side_effect=error),
            self.assertRaisesRegex(
                SystemExit,
                "Could not download documentation from https://example.test/missing.md: HTTP Error 404",
            ),
        ):
            docs_pages.load_doc_source({"content_url": "https://example.test/missing.md"})

    def test_load_doc_source_reports_remote_timeout(self) -> None:
        with (
            patch.object(docs_pages, "urlopen", side_effect=TimeoutError("timed out")),
            self.assertRaisesRegex(
                SystemExit,
                "Could not download documentation from https://example.test/README.md: timed out",
            ),
        ):
            docs_pages.load_doc_source({"content_url": "https://example.test/README.md"})

    def test_load_doc_source_reports_invalid_remote_utf8(self) -> None:
        response = MagicMock()
        response.__enter__.return_value.read.return_value = b"\xff"

        with (
            patch.object(docs_pages, "urlopen", return_value=response),
            self.assertRaisesRegex(SystemExit, "Could not download documentation"),
        ):
            docs_pages.load_doc_source({"content_url": "https://example.test/README.md"})

    def test_load_doc_source_reads_local_documentation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "guide.md").write_text("# Local guide\n", encoding="utf-8")

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                source = docs_pages.load_doc_source({"filename": "guide.md"})

        self.assertEqual(source, "# Local guide\n")

    def test_load_doc_source_propagates_a_missing_local_document(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(docs_pages, "DOCS_DIR", Path(directory)),
            self.assertRaises(FileNotFoundError),
        ):
            docs_pages.load_doc_source({"filename": "missing.md"})

    def test_raw_github_link_escapes_a_custom_source_url(self) -> None:
        result = docs_pages.raw_github_link(
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
                patch.object(docs_pages, "DOCS_DIR", docs_dir),
                patch.object(docs_pages, "DOC_FILENAME_TO_SLUG", {"other.md": "other"}),
            ):
                title, description, content = docs_pages.render_doc(doc)

        self.assertEqual(title, "Guide")
        self.assertEqual(description, "Read other before starting.")
        self.assertIn('<a href="other.html"><code>other</code></a>', content)
        self.assertIn('href="https://example.test/source"', content)

    def test_render_doc_uses_filename_when_markdown_has_no_title(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "notes.md").write_text("Opening paragraph.\n", encoding="utf-8")
            doc = {"filename": "notes.md", "slug": "notes", "kind": "markdown"}

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                title, description, _ = docs_pages.render_doc(doc)

        self.assertEqual(title, "notes.md")
        self.assertEqual(description, "Opening paragraph.")

    def test_render_doc_keeps_a_non_title_heading_in_the_markdown_body(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "notes.md").write_text(
                "## Overview\n\nOpening paragraph.\n", encoding="utf-8"
            )

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                title, description, content = docs_pages.render_doc(
                    {"filename": "notes.md", "slug": "notes", "kind": "markdown"}
                )

        self.assertEqual(title, "notes.md")
        self.assertEqual(description, "Opening paragraph.")
        self.assertIn("<h2>Overview</h2>", content)

    def test_render_doc_handles_an_empty_markdown_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "empty.md").write_text("", encoding="utf-8")

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                title, description, content = docs_pages.render_doc(
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
                patch.object(docs_pages, "DOCS_DIR", docs_dir),
                patch.object(docs_pages, "GITHUB_BLOB_BASE", "https://example.test/repo"),
            ):
                _, description, content = docs_pages.render_doc(
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

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                schema = docs_pages.render_doc(
                    {"filename": "schema.json", "slug": "schema", "kind": "json-schema"}
                )
                plain = docs_pages.render_doc(
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

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                title, description, content = docs_pages.render_doc(
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

    def test_render_doc_uses_filename_defaults_for_schema_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "schema.json").write_text('{"type":"string"}', encoding="utf-8")

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                title, description, content = docs_pages.render_doc(
                    {"filename": "schema.json", "slug": "schema", "kind": "json-schema"}
                )

        self.assertEqual(title, "schema.json")
        self.assertEqual(description, "")
        self.assertIn('<pre class="code-block language-json" tabindex="0">', content)
        self.assertIn("type", content)
        self.assertIn("string", content)

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

            with patch.object(docs_pages, "DOCS_DIR", docs_dir):
                title, description, content = docs_pages.render_doc(doc)

        self.assertEqual((title, description), ("Schema", "Short page summary."))
        self.assertIn("A very long schema description.", content)

    def test_render_doc_propagates_invalid_json_schema_input(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            docs_dir = Path(directory)
            (docs_dir / "schema.json").write_text("{not valid json", encoding="utf-8")

            with (
                patch.object(docs_pages, "DOCS_DIR", docs_dir),
                self.assertRaisesRegex(ValueError, "Expecting property name"),
            ):
                docs_pages.render_doc({"filename": "schema.json", "slug": "schema", "kind": "json-schema"})

    def test_render_docs_list_items_escapes_content_and_applies_prefix(self) -> None:
        docs = [{"slug": "guide", "blurb": "Use <carefully> & safely"}]
        results = {"guide": {"title": "Guide & reference"}}

        with patch.object(docs_pages, "DOCS", docs):
            root_output = docs_pages.render_docs_list_items(results, None)
            docs_output = docs_pages.render_docs_list_items(results, "docs")

        self.assertIn('href="docs/guide.html"', root_output)
        self.assertIn("Guide &amp; reference", root_output)
        self.assertIn("Use &lt;carefully&gt; &amp; safely", root_output)
        self.assertIn('href="guide.html"', docs_output)

    def test_render_docs_list_items_links_across_to_a_different_prefix(self) -> None:
        docs = [{"slug": "papyrus-lint-schema", "blurb": "Schema", "repo_dir": "schema"}]
        results = {"papyrus-lint-schema": {"title": "Schema"}}

        with patch.object(docs_pages, "DOCS", docs):
            root_output = docs_pages.render_docs_list_items(results, None)
            docs_output = docs_pages.render_docs_list_items(results, "docs")
            schema_output = docs_pages.render_docs_list_items(results, "schema")

        self.assertIn('href="schema/papyrus-lint-schema.html"', root_output)
        self.assertIn('href="../schema/papyrus-lint-schema.html"', docs_output)
        self.assertIn('href="papyrus-lint-schema.html"', schema_output)

    def test_render_docs_list_items_preserves_configured_document_order(self) -> None:
        docs = [
            {"slug": "second", "blurb": "Second blurb"},
            {"slug": "first", "blurb": "First blurb"},
        ]
        results = {
            "first": {"title": "First"},
            "second": {"title": "Second"},
        }

        with patch.object(docs_pages, "DOCS", docs):
            output = docs_pages.render_docs_list_items(results, "")

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
                patch.object(docs_pages, "PAGES_DIR", pages_dir),
                patch.object(docs_pages, "DOCS", docs),
                patch.object(docs_pages, "SITE_URL", "https://example.test/"),
            ):
                docs_pages.build_doc_pages(out_dir, results)

            detail = (out_dir / "docs" / "guide.html").read_text(encoding="utf-8")
            index = (out_dir / "docs" / "index.html").read_text(encoding="utf-8")

        self.assertIn("<title>Guide &amp; help</title>", detail)
        self.assertIn('content="Use &quot;care&quot; &amp; attention"', detail)
        self.assertIn('href=https://example.test/docs/guide.html', detail)
        self.assertIn("<p>Contents</p>", detail)
        self.assertIn('href="guide.html"', index)
        self.assertIn("A useful guide", index)


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
                patch.object(docs_pages, "PAGES_DIR", pages_dir),
                patch.object(docs_pages, "urlopen", return_value=response),
            ):
                docs_pages.build_action_page(out_dir, version="v1.0.0")

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
                patch.object(docs_pages, "PAGES_DIR", pages_dir),
                patch.object(docs_pages, "urlopen", return_value=response),
                self.assertRaisesRegex(SystemExit, "missing marker <!--ACTION_DESCRIPTION-->"),
            ):
                docs_pages.build_action_page(out_dir)

            self.assertFalse((out_dir / "action.html").exists())


class RepositoryDocsConfigurationTest(unittest.TestCase):
    """Keep docs_pages.py's checked-in inputs synchronized with the repository."""

    def test_document_manifest_has_unique_slugs_and_readable_local_sources(self) -> None:
        docs_with_filenames = [doc for doc in docs_pages.DOCS if "filename" in doc]
        slugs = [doc["slug"] for doc in docs_pages.DOCS]
        filenames = [doc["filename"] for doc in docs_with_filenames]

        self.assertEqual(len(slugs), len(set(slugs)), "documentation slugs must be unique")
        self.assertEqual(len(filenames), len(set(filenames)), "documentation sources must be unique")
        for doc in docs_with_filenames:
            source = doc.get("source_dir", docs_pages.DOCS_DIR) / doc["filename"]
            with self.subTest(filename=doc["filename"]):
                self.assertTrue(source.is_file(), f"missing documentation source: {source}")
                self.assertTrue(source.read_text(encoding="utf-8").strip())

    def test_every_local_document_renders_with_metadata_and_a_source_link(self) -> None:
        for doc in docs_pages.DOCS:
            if "content_url" in doc:
                continue
            with self.subTest(slug=doc["slug"]):
                title, description, content = docs_pages.render_doc(doc)

                self.assertTrue(title.strip())
                self.assertTrue(description.strip())
                self.assertTrue(content.strip())
                self.assertIn("View raw source on GitHub", content)
                self.assertIn(f"/{doc.get('repo_dir', 'docs')}/{doc['filename']}", content)


if __name__ == "__main__":
    unittest.main()
