"""Tests for the dependency-free GitHub Pages site builder."""

from __future__ import annotations

import json
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import MagicMock, patch
from urllib.error import URLError

from PIL import Image

from pages import build as page_builder


class PublishedSchemaTest(unittest.TestCase):
    def test_ai_export_external_diagnostic_fields_require_each_other(self) -> None:
        schema = json.loads(
            (page_builder.DOCS_DIR / "papyrus-lint-ai-export.v2.schema.json").read_text(encoding="utf-8")
        )

        self.assertEqual(
            schema["$defs"]["diagnostic"]["dependentRequired"],
            {"external": ["source"], "source": ["external"]},
        )


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

    def test_extract_section_returns_an_empty_section_at_end_of_document(self) -> None:
        self.assertEqual(page_builder.extract_section(["# Guide", "## Empty"], "Empty", level=2), [])

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

    def test_render_inline_leaves_plain_text_unchanged(self) -> None:
        self.assertEqual(
            page_builder.render_inline("Papyrus source uses properties and events."),
            "Papyrus source uses properties and events.",
        )

    def test_render_inline_escapes_markup_in_link_labels_and_code(self) -> None:
        rendered = page_builder.render_inline(
            '[<Guide & notes>](guide.html) and `<script>alert("x")</script>`'
        )

        self.assertEqual(
            rendered,
            '<a href="guide.html">&lt;Guide &amp; notes&gt;</a> and '
            '<code>&lt;script&gt;alert("x")&lt;/script&gt;</code>',
        )
        self.assertNotIn("<script>", rendered)

    def test_render_inline_calls_the_link_rewriter_for_every_link_only(self) -> None:
        rewritten = []

        def rewrite(href: str) -> str:
            rewritten.append(href)
            return f"published/{href}"

        rendered = page_builder.render_inline(
            "See [one](one.md), `two.md`, and [three](three.md).", rewrite
        )

        self.assertEqual(rewritten, ["one.md", "three.md"])
        self.assertEqual(
            rendered,
            'See <a href="published/one.md">one</a>, <code>two.md</code>, and '
            '<a href="published/three.md">three</a>.',
        )

    def test_split_table_row_preserves_escaped_pipes(self) -> None:
        self.assertEqual(
            page_builder.split_table_row(r"| Name | a \| b | yes |"),
            ["Name", "a | b", "yes"],
        )

    def test_split_table_row_accepts_rows_without_outer_pipes(self) -> None:
        self.assertEqual(
            page_builder.split_table_row("Name | Description | Auto-Fix"),
            ["Name", "Description", "Auto-Fix"],
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

    def test_render_lint_table_treats_whitespace_only_auto_fix_as_disabled(self) -> None:
        result = page_builder.render_lint_table(
            [
                "| Lint | Description | Auto-Fix |",
                "| --- | --- | --- |",
                "| safety | Still linted |    |",
            ]
        )

        self.assertNotIn('class="fix-yes"', result)
        self.assertIn("<td></td>", result)

    def test_render_lint_table_escapes_headers_and_row_content(self) -> None:
        result = page_builder.render_lint_table(
            [
                "| <Lint> | Description & impact | Auto-Fix |",
                "| --- | --- | --- |",
                '| <unsafe> | Never render <script> or "quotes" | Yes |',
            ]
        )

        self.assertIn("<th>&lt;Lint&gt;</th>", result)
        self.assertIn("<th>Description &amp; impact</th>", result)
        self.assertIn("<td>&lt;unsafe&gt;</td>", result)
        self.assertIn("Never render &lt;script&gt; or \"quotes\"", result)
        self.assertNotIn("<script>", result)

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

    def test_first_code_block_ignores_prose_and_later_fenced_blocks(self) -> None:
        self.assertEqual(
            page_builder.first_code_block(
                ["before", "```shell", "first", "```", "between", "```", "second", "```"]
            ),
            "first",
        )

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
            '<pre class="code-block language-yaml" tabindex="0"><code>unsafe: &lt;value&gt;</code></pre>',
            result,
        )

    def test_highlight_code_marks_tokens_and_escapes_untrusted_source(self) -> None:
        result = page_builder.highlight_code(
            '{"enabled": true, "count": 12, "unsafe": "<script>"}', "json"
        )

        self.assertIn('<span class="str">&quot;enabled&quot;</span>', result)
        self.assertIn('<span class="kw">true</span>', result)
        self.assertIn('<span class="num">12</span>', result)
        self.assertIn('&lt;script&gt;', result)
        self.assertNotIn('<script>', result)

    def test_highlight_code_supports_aliases_and_plain_text_fallback(self) -> None:
        self.assertIn('<span class="cm"># note</span>', page_builder.highlight_code("# note", "yml"))
        self.assertEqual(page_builder.highlight_code("<unsafe>", "text"), "&lt;unsafe&gt;")

    def test_highlight_code_marks_papyrus_keywords_types_strings_and_comments(self) -> None:
        result = page_builder.highlight_code(
            'ScriptName Example\n\n; note\nFunction Greet(Int aiCount)\n    Debug.Trace("hi")\nEndFunction',
            "papyrus",
        )

        self.assertIn('<span class="kw">ScriptName</span>', result)
        self.assertIn('<span class="kw">Function</span>', result)
        self.assertIn('<span class="kw">EndFunction</span>', result)
        self.assertIn('<span class="ty">Int</span>', result)
        self.assertIn('<span class="cm">; note</span>', result)
        self.assertIn('<span class="str">&quot;hi&quot;</span>', result)

    def test_highlight_code_normalizes_language_names_case_insensitively(self) -> None:
        result = page_builder.highlight_code("if true; then echo 12; fi", "BASH")

        self.assertIn('<span class="kw">if</span>', result)
        self.assertIn('<span class="kw">then</span>', result)
        self.assertIn('<span class="kw">fi</span>', result)

    def test_highlight_code_marks_json_numbers_literals_and_escaped_strings(self) -> None:
        result = page_builder.highlight_code(
            r'{"message": "say \"hello\"", "ratio": -1.25e+3, "missing": null}',
            "json-schema",
        )

        self.assertIn('<span class="str">&quot;message&quot;</span>', result)
        self.assertIn(
            '<span class="str">&quot;say \\&quot;hello\\&quot;&quot;</span>', result
        )
        self.assertIn('<span class="num">-1.25e+3</span>', result)
        self.assertIn('<span class="kw">null</span>', result)

    def test_highlight_code_marks_yaml_tokens_without_coloring_number_like_words(self) -> None:
        result = page_builder.highlight_code(
            'enabled: yes\ncount: -12.5\nrelease: v1.2\nlabel: "safe" # note', "yaml"
        )

        self.assertIn('<span class="kw">yes</span>', result)
        self.assertIn('<span class="num">-12.5</span>', result)
        self.assertIn('<span class="str">&quot;safe&quot;</span>', result)
        self.assertIn('<span class="cm"># note</span>', result)
        self.assertNotIn('v<span class="num">1.2</span>', result)

    def test_highlight_code_marks_shell_and_bbcode_specific_syntax(self) -> None:
        shell = page_builder.highlight_code(
            "for item in 'two words'; do echo \"$item\"; done # note", "sh"
        )
        bbcode = page_builder.highlight_code("[b]Safe[/b] <unsafe>", "bbcode")

        for keyword in ("for", "in", "do", "done"):
            self.assertIn(f'<span class="kw">{keyword}</span>', shell)
        self.assertIn('<span class="str">&#x27;two words&#x27;</span>', shell)
        self.assertIn('<span class="cm"># note</span>', shell)
        self.assertEqual(
            bbcode,
            '<span class="tag">[b]</span>Safe<span class="tag">[/b]</span> &lt;unsafe&gt;',
        )

    def test_markdown_to_html_flushes_a_final_paragraph(self) -> None:
        result = page_builder.markdown_to_html(["A paragraph", "continued without a blank line."])

        self.assertEqual(result, "<p>A paragraph continued without a blank line.</p>")

    def test_markdown_to_html_rewrites_links_in_headings_and_paragraphs(self) -> None:
        rewritten = []

        def rewrite(href: str) -> str:
            rewritten.append(href)
            return f"published/{href}"

        result = page_builder.markdown_to_html(
            ["## [Setup](setup.md)", "Read the [guide](guide.md)."], rewrite
        )

        self.assertEqual(rewritten, ["setup.md", "guide.md"])
        self.assertEqual(
            result,
            '<h2><a href="published/setup.md">Setup</a></h2>\n'
            '<p>Read the <a href="published/guide.md">guide</a>.</p>',
        )

    def test_markdown_to_html_accepts_an_unclosed_final_code_fence(self) -> None:
        result = page_builder.markdown_to_html(["```text", "first", "  second"])

        self.assertEqual(
            result,
            '<pre class="code-block language-text" tabindex="0"><code>first\n  second</code></pre>',
        )

    def test_markdown_to_html_escapes_headings_and_code_blocks(self) -> None:
        result = page_builder.markdown_to_html(
            ["###### <Advanced> & **safe**", "", "```", '<script data-x="1">', "```"]
        )

        self.assertEqual(
            result,
            "<h6>&lt;Advanced&gt; &amp; <strong>safe</strong></h6>\n"
            '<pre class="code-block" tabindex="0"><code>'
            '&lt;script data-x=&quot;1&quot;&gt;</code></pre>',
        )

    def test_markdown_to_html_escapes_an_untrusted_fence_language(self) -> None:
        result = page_builder.markdown_to_html(
            ['```yaml" onmouseover="alert(1)', "enabled: true", "```"]
        )

        self.assertIn('class="code-block language-yaml&quot;" tabindex="0"', result)
        self.assertNotIn("onmouseover=", result)
        self.assertIn("enabled: true", result)

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

    def test_first_paragraph_stops_when_code_follows_prose(self) -> None:
        lines = ["Introductory text.", "```text", "not part of the description", "```"]

        self.assertEqual(page_builder.first_paragraph(lines), "Introductory text.")

    def test_first_paragraph_stops_when_a_heading_follows_prose(self) -> None:
        lines = ["Opening summary.", "continues here.", "## Details", "Not part of the summary."]

        self.assertEqual(
            page_builder.first_paragraph(lines),
            "Opening summary. continues here.",
        )


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

    def test_minify_html_preserves_comment_like_text_inside_pre_blocks(self) -> None:
        pre_block = "<pre><!-- example syntax -->\n  value</pre>"

        result = page_builder.minify_html(f"<!-- remove me --><main>{pre_block}</main>")

        self.assertEqual(result, f"<main>{pre_block}</main>")

    def test_minify_html_recognizes_pre_tags_case_insensitively(self) -> None:
        pre_block = "<PRE class=\"example\">first\n    second</PRE>"

        result = page_builder.minify_html(f"<main>\n  {pre_block}\n</main>")

        self.assertIn(pre_block, result)
        self.assertEqual(result.count("\n"), 1)

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

    def test_minify_css_handles_empty_and_comment_only_stylesheets(self) -> None:
        self.assertEqual(page_builder.minify_css(""), "")
        self.assertEqual(page_builder.minify_css(" /* generated stylesheet */ \n"), "")

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

    def test_wrap_images_with_modern_sources_handles_multiple_attributes_and_extensions(self) -> None:
        page_html = (
            '<img src="assets/first.png" alt="First">'
            '<img loading="lazy" src="assets/second.jpg" class="shot">'
        )

        with patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"first.png", "second.jpg"}):
            result = page_builder.wrap_images_with_modern_sources(page_html)

        self.assertEqual(result.count("<picture>"), 2)
        self.assertIn('srcset="assets/first.avif"', result)
        self.assertIn('srcset="assets/second.webp"', result)
        self.assertIn('<img loading="lazy" src="assets/second.jpg" class="shot">', result)


class RepositoryConfigurationTest(unittest.TestCase):
    """Keep build.py's checked-in inputs synchronized with its manifest data."""

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

    def test_index_template_has_exactly_one_marker_for_each_lint_category(self) -> None:
        template = (page_builder.PAGES_DIR / "index.template.html").read_text(encoding="utf-8")

        for category in page_builder.LINT_CATEGORIES:
            marker = f"<!--LINT_TABLE:{category}-->"
            with self.subTest(category=category):
                self.assertEqual(template.count(marker), 1)


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
            for category in page_builder.LINT_CATEGORIES:
                with self.subTest(lint_category=category):
                    self.assertIn(f"<h3>{category}</h3>", index)
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
            published_schemas = {
                path.name: path.read_bytes() for path in (out_dir / "schema").glob("*.json")
            }
            self.assertEqual(published_schemas, expected_schemas)

            self.assertEqual(
                (out_dir / "CNAME").read_text(encoding="utf-8"),
                page_builder.CNAME_FILE.read_text(encoding="utf-8"),
            )
            for output_name in page_builder.ASSETS:
                with self.subTest(asset=output_name):
                    self.assertTrue((out_dir / "assets" / output_name).is_file())
            for output_name in page_builder.MODERN_FORMAT_ASSETS:
                stem = Path(output_name).stem
                with self.subTest(modern_asset=output_name):
                    self.assertTrue((out_dir / "assets" / f"{stem}.webp").is_file())
                    self.assertTrue((out_dir / "assets" / f"{stem}.avif").is_file())


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
            (pages_dir / "styles.css").write_text("main { color: red; }", encoding="utf-8")
            (pages_dir / "theme.js").write_text("/* theme js */", encoding="utf-8")
            (pages_dir / "downloads.js").write_text("/* downloads js */", encoding="utf-8")
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
                patch.object(page_builder, "LINT_CATEGORIES", ["Formatting"]),
                patch.object(page_builder, "DOCS", []),
                patch.object(
                    page_builder,
                    "ASSETS",
                    {"copied.png": copied_asset, "screenshot.png": screenshot_asset},
                ),
                patch.object(page_builder, "MODERN_FORMAT_ASSETS", {"screenshot.png"}),
                patch.object(page_builder, "urlopen", return_value=action_response),
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
            self.assertEqual((out_dir / "theme.js").read_text(encoding="utf-8"), "/* theme js */")
            self.assertEqual(
                (out_dir / "downloads.js").read_text(encoding="utf-8"), "/* downloads js */"
            )
            self.assertFalse((out_dir / "stale.txt").exists())
            self.assertTrue((out_dir / "docs" / "index.html").exists())

            videos_output = (out_dir / "videos.html").read_text(encoding="utf-8")
            self.assertIn("youtube-nocookie.com/embed/", videos_output)
            self.assertNotIn("<!--VIDEOS_LIST-->", videos_output)

            action_output = (out_dir / "action.html").read_text(encoding="utf-8")
            self.assertIn("Papyrus Lint Action", action_output)
            self.assertIn("Lints pull requests automatically.", action_output)
            self.assertNotIn("<!--ACTION_TITLE-->", action_output)

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
            self.assertIn(f"<loc>{page_builder.SITE_URL}videos.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}coverage.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}imprint.html</loc>", sitemap_output)
            self.assertIn(f"<loc>{page_builder.SITE_URL}docs/index.html</loc>", sitemap_output)

            coverage_output = (out_dir / "coverage.html").read_text(encoding="utf-8")
            self.assertIn("v1.2.3", coverage_output)
            self.assertIn("Coverage data isn't available for this build.", coverage_output)

            imprint_output = (out_dir / "imprint.html").read_text(encoding="utf-8")
            self.assertIn("Legal Notice", imprint_output)

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
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
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
                "<!--LINT_TABLE:Formatting--><!--CLI_EXAMPLES-->", encoding="utf-8"
            )

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "LINT_CATEGORIES", ["Formatting"]),
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
    def test_load_coverage_summary_loads_the_shared_ci_module(self) -> None:
        coverage_summary = page_builder.load_coverage_summary()

        self.assertEqual(coverage_summary.pct(7, 8), "87.5%")
        self.assertIn(
            ("Pages (site builder)", "pages-coverage/lcov.info"),
            next(parts for label, parts in coverage_summary.MODULES if label == "Tooling"),
        )

    def test_normalize_source_path_strips_a_ci_checkout_prefix(self) -> None:
        self.assertEqual(
            page_builder.normalize_source_path(
                "/home/runner/work/papyrus-lint/papyrus-lint/app/crates/papyrus-parser/src/lexer.rs"
            ),
            "app/crates/papyrus-parser/src/lexer.rs",
        )

    def test_normalize_source_path_leaves_an_already_relative_path_unchanged(self) -> None:
        self.assertEqual(page_builder.normalize_source_path("src/main.ts"), "src/main.ts")

    def test_normalize_source_path_normalizes_windows_style_separators(self) -> None:
        self.assertEqual(
            page_builder.normalize_source_path(
                r"C:\work\papyrus-lint\papyrus-lint\app\crates\papyrus-lints\src\lib.rs"
            ),
            "app/crates/papyrus-lints/src/lib.rs",
        )

    def test_parse_lcov_files_returns_none_for_a_missing_report(self) -> None:
        self.assertIsNone(page_builder.parse_lcov_files(Path("does-not-exist.info")))

    def test_parse_lcov_files_returns_per_file_records(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:/home/runner/work/papyrus-lint/papyrus-lint/app/src/one.rs\n"
                "LF:10\nLH:8\nend_of_record\n"
                "SF:app/src/two.rs\n"
                "LF:4\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual(
                page_builder.parse_lcov_files(report),
                [("app/src/one.rs", 10, 8), ("app/src/two.rs", 4, 1)],
            )

    def test_parse_lcov_files_ignores_a_record_without_a_source_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text("LF:5\nLH:5\nend_of_record\n", encoding="utf-8")

            self.assertEqual(page_builder.parse_lcov_files(report), [])

    def test_parse_lcov_files_accumulates_repeated_summary_lines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:src/generated.rs\nLF:3\nLF:2\nLH:1\nLH:2\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual(
                page_builder.parse_lcov_files(report),
                [("src/generated.rs", 5, 3)],
            )

    def test_parse_lcov_files_resets_counts_when_a_new_record_starts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:incomplete.rs\nLF:50\nLH:40\n"
                "SF:complete.rs\nLF:4\nLH:3\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual(
                page_builder.parse_lcov_files(report),
                [("complete.rs", 4, 3)],
            )

    def test_parse_lcov_files_discards_an_unterminated_record(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:complete.rs\nLF:2\nLH:2\nend_of_record\n"
                "SF:partial.rs\nLF:10\nLH:9\n",
                encoding="utf-8",
            )

            result = page_builder.parse_lcov_files(report)

        self.assertEqual(result, [("complete.rs", 2, 2)])

    def test_parse_lcov_files_replaces_invalid_utf8_in_source_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_bytes(b"SF:src/invalid-\xff.rs\nLF:1\nLH:1\nend_of_record\n")

            result = page_builder.parse_lcov_files(report)

        self.assertEqual(result, [("src/invalid-\ufffd.rs", 1, 1)])

    def test_parse_lcov_files_handles_empty_and_zero_coverage_records(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "TN:site-builder\n"
                "SF:pages/empty.py\nend_of_record\n"
                "SF:pages/untested.py\nLF:3\nLH:0\nBRF:2\nBRH:0\nend_of_record\n",
                encoding="utf-8",
            )

            result = page_builder.parse_lcov_files(report)

        self.assertEqual(
            result,
            [("pages/empty.py", 0, 0), ("pages/untested.py", 3, 0)],
        )

    def test_render_coverage_table_renders_rows_with_percentage_and_counts(self) -> None:
        coverage_summary = page_builder.load_coverage_summary()
        result = page_builder.render_coverage_table([("src/one.rs", 10, 8)], coverage_summary)

        self.assertIn("<code>src/one.rs</code>", result)
        self.assertIn("<td>80.0%</td>", result)
        self.assertIn("<td>8/10</td>", result)

    def test_render_coverage_table_handles_no_rows(self) -> None:
        coverage_summary = page_builder.load_coverage_summary()
        result = page_builder.render_coverage_table([], coverage_summary)

        self.assertEqual(result, '<p class="section-intro">No files reported.</p>')

    def test_render_coverage_table_escapes_source_file_names(self) -> None:
        coverage_summary = page_builder.load_coverage_summary()

        result = page_builder.render_coverage_table(
            [('src/<unsafe>&"file".rs', 2, 1)], coverage_summary
        )

        self.assertIn("<code>src/&lt;unsafe&gt;&amp;&quot;file&quot;.rs</code>", result)
        self.assertNotIn("<unsafe>", result)

    def test_render_coverage_table_preserves_the_supplied_row_order(self) -> None:
        coverage_summary = page_builder.load_coverage_summary()

        result = page_builder.render_coverage_table(
            [("lowest.rs", 10, 1), ("middle.rs", 10, 5), ("highest.rs", 10, 9)],
            coverage_summary,
        )

        self.assertLess(result.index("lowest.rs"), result.index("middle.rs"))
        self.assertLess(result.index("middle.rs"), result.index("highest.rs"))

    def test_build_coverage_content_groups_by_module_and_sorts_worst_first(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            coverage_summary = page_builder.load_coverage_summary()
            coverage_summary.MODULES = [
                (
                    "Combined",
                    [
                        ("first-part", "first/lcov.info"),
                        ("second-part", "second/lcov.info"),
                    ],
                ),
                ("Missing", [("absent", "absent/lcov.info")]),
            ]
            first_dir = root / "first"
            first_dir.mkdir()
            (first_dir / "lcov.info").write_text(
                "SF:good.rs\nLF:10\nLH:10\nend_of_record\n"
                "SF:bad.rs\nLF:10\nLH:2\nend_of_record\n",
                encoding="utf-8",
            )
            second_dir = root / "second"
            second_dir.mkdir()
            (second_dir / "lcov.info").write_text("SF:only.rs\nLF:4\nLH:4\nend_of_record\n", encoding="utf-8")

            result = page_builder.build_coverage_content(root, coverage_summary)

        self.assertIn("Combined", result)
        self.assertIn("Missing", result)
        self.assertIn("No report.", result)
        self.assertLess(result.index("bad.rs"), result.index("good.rs"))
        self.assertIn("Total line coverage: <strong>66.7%</strong> (16/24)", result)

    def test_build_coverage_content_reports_na_when_nothing_is_available(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            coverage_summary = page_builder.load_coverage_summary()
            coverage_summary.MODULES = [("Missing", [("absent", "absent/lcov.info")])]

            result = page_builder.build_coverage_content(root, coverage_summary)

        self.assertIn("Total line coverage: <strong>n/a</strong> (0/0)", result)

    def test_render_coverage_entry_recurses_through_nested_groups(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report_dir = root / "child"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text(
                "SF:empty.rs\nLF:0\nLH:0\nend_of_record\n",
                encoding="utf-8",
            )
            coverage_summary = page_builder.load_coverage_summary()

            found, hit, any_report, result = page_builder.render_coverage_entry(
                root,
                "parent",
                [("available", "child/lcov.info"), ("missing", "missing/lcov.info")],
                coverage_summary,
            )

        self.assertEqual((found, hit, any_report), (0, 0, True))
        self.assertIn("<h3>available — n/a (0/0)</h3>", result)
        self.assertIn("<code>empty.rs</code>", result)
        self.assertIn("<h3>missing — n/a (0/0)</h3>", result)
        self.assertIn("No report.", result)

    def test_render_coverage_entry_omits_a_redundant_heading_for_one_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report_dir = root / "only"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text(
                "SF:src/only.py\nLF:4\nLH:3\nend_of_record\n",
                encoding="utf-8",
            )
            coverage_summary = page_builder.load_coverage_summary()

            found, hit, any_report, result = page_builder.render_coverage_entry(
                root, "parent", [("only child", "only/lcov.info")], coverage_summary
            )

        self.assertEqual((found, hit, any_report), (4, 3, True))
        self.assertNotIn("<h3>", result)
        self.assertIn("<code>src/only.py</code>", result)

    def test_render_coverage_entry_sorts_equal_percentages_by_source_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "lcov.info"
            report.write_text(
                "SF:pages/z-last.py\nLF:4\nLH:2\nend_of_record\n"
                "SF:pages/a-first.py\nLF:2\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )
            coverage_summary = page_builder.load_coverage_summary()

            found, hit, any_report, result = page_builder.render_coverage_entry(
                root, "Pages", "lcov.info", coverage_summary
            )

        self.assertEqual((found, hit, any_report), (6, 3, True))
        self.assertLess(result.index("pages/a-first.py"), result.index("pages/z-last.py"))

    def test_render_coverage_entry_escapes_nested_group_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            coverage_summary = page_builder.load_coverage_summary()

            _found, _hit, _any_report, result = page_builder.render_coverage_entry(
                Path(directory),
                "parent",
                [("<available>", "missing-one.info"), ("safe & sound", "missing-two.info")],
                coverage_summary,
            )

        self.assertIn("<h3>&lt;available&gt; — n/a (0/0)</h3>", result)
        self.assertIn("<h3>safe &amp; sound — n/a (0/0)</h3>", result)
        self.assertNotIn("<available>", result)

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
