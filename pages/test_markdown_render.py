"""Tests for the small-subset Markdown-to-HTML conversion used by build.py."""

from __future__ import annotations

import unittest
from unittest.mock import MagicMock

from pages import markdown_render


class ExtractSectionTest(unittest.TestCase):
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
            markdown_render.extract_section(lines, "Wanted", level=2),
            ["intro", "### Child", "child text"],
        )

    def test_extract_section_rejects_a_missing_heading(self) -> None:
        with self.assertRaisesRegex(SystemExit, "heading not found"):
            markdown_render.extract_section(["## Present"], "Missing", level=2)

    def test_extract_section_ignores_same_text_at_a_different_level(self) -> None:
        lines = ["# Wanted", "wrong", "## Wanted", "right", "### Child", "also right"]

        self.assertEqual(
            markdown_render.extract_section(lines, "Wanted", level=2),
            ["right", "### Child", "also right"],
        )

    def test_extract_section_returns_an_empty_section_at_end_of_document(self) -> None:
        self.assertEqual(markdown_render.extract_section(["# Guide", "## Empty"], "Empty", level=2), [])

    def test_extract_section_includes_deeper_headings_until_its_parent_closes(self) -> None:
        lines = [
            "## Wanted",
            "### First child",
            "child body",
            "#### Grandchild",
            "grandchild body",
            "# Next top-level section",
        ]

        self.assertEqual(
            markdown_render.extract_section(lines, "Wanted", level=2),
            ["### First child", "child body", "#### Grandchild", "grandchild body"],
        )


class RenderInlineTest(unittest.TestCase):
    def test_render_inline_converts_supported_markdown_and_escapes_html(self) -> None:
        rendered = markdown_render.render_inline(
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

        rendered = markdown_render.render_inline("[Guide](guide.md)", rewrite)

        self.assertEqual(seen_hrefs, ["guide.md"])
        self.assertEqual(
            rendered,
            '<a href="docs/guide.md?label=&quot;read&quot;&amp;mode=full">Guide</a>',
        )

    def test_render_inline_leaves_plain_text_unchanged(self) -> None:
        self.assertEqual(
            markdown_render.render_inline("Papyrus source uses properties and events."),
            "Papyrus source uses properties and events.",
        )

    def test_render_inline_escapes_markup_in_link_labels_and_code(self) -> None:
        rendered = markdown_render.render_inline(
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

        rendered = markdown_render.render_inline(
            "See [one](one.md), `two.md`, and [three](three.md).", rewrite
        )

        self.assertEqual(rewritten, ["one.md", "three.md"])
        self.assertEqual(
            rendered,
            'See <a href="published/one.md">one</a>, <code>two.md</code>, and '
            '<a href="published/three.md">three</a>.',
        )

    def test_render_inline_escapes_ampersands_in_rewritten_link_targets_once(self) -> None:
        result = markdown_render.render_inline(
            "[filtered](search.md)",
            lambda _href: "results.html?kind=lint&state=open",
        )

        self.assertEqual(
            result,
            '<a href="results.html?kind=lint&amp;state=open">filtered</a>',
        )

    def test_render_inline_escapes_quotes_without_treating_plain_text_as_an_attribute(self) -> None:
        result = markdown_render.render_inline('The "quoted" value links to [docs](guide.md).')

        self.assertEqual(
            result,
            'The "quoted" value links to <a href="guide.md">docs</a>.',
        )

    def test_render_inline_does_not_rewrite_an_unclosed_markdown_link(self) -> None:
        rewrite = MagicMock()

        result = markdown_render.render_inline("Read [the guide](guide.md", rewrite)

        self.assertEqual(result, "Read [the guide](guide.md")
        rewrite.assert_not_called()


class FirstCodeBlockTest(unittest.TestCase):
    def test_first_code_block_returns_contents(self) -> None:
        self.assertEqual(
            markdown_render.first_code_block(
                ["prose", "```console", "command --flag", "second line", "```"]
            ),
            "command --flag\nsecond line",
        )

    def test_first_code_block_rejects_missing_or_unterminated_fence(self) -> None:
        with self.assertRaisesRegex(SystemExit, "fenced code block"):
            markdown_render.first_code_block(["prose"])
        with self.assertRaisesRegex(SystemExit, "unterminated"):
            markdown_render.first_code_block(["```console", "command"])

    def test_first_code_block_ignores_prose_and_later_fenced_blocks(self) -> None:
        self.assertEqual(
            markdown_render.first_code_block(
                ["before", "```shell", "first", "```", "between", "```", "second", "```"]
            ),
            "first",
        )

    def test_first_code_block_accepts_indented_fences_and_preserves_code_indent(self) -> None:
        lines = ["prose", "   ```console", "  command --flag", "   ```"]

        self.assertEqual(markdown_render.first_code_block(lines), "  command --flag")


class StripMarkdownInlineTest(unittest.TestCase):
    def test_strip_markdown_inline_produces_plain_text(self) -> None:
        self.assertEqual(
            markdown_render.strip_markdown_inline(
                "Read **the [`configuration`](config.html)** for `details`."
            ),
            "Read the configuration for details.",
        )


class FirstParagraphTest(unittest.TestCase):
    def test_first_paragraph_skips_headings_and_joins_wrapped_lines(self) -> None:
        lines = ["# Title", "", "First line with `code`", "continues here.", "", "Second paragraph."]

        self.assertEqual(
            markdown_render.first_paragraph(lines),
            "First line with `code` continues here.",
        )

    def test_first_paragraph_skips_a_leading_fenced_example(self) -> None:
        lines = [
            "# Guide",
            "```papyrus",
            "ScriptName Example",
            "",
            "; blank lines inside the example are not prose",
            "```",
            "## Overview",
            "The first real paragraph",
            "continues here.",
            "",
            "A later paragraph.",
        ]

        self.assertEqual(
            markdown_render.first_paragraph(lines),
            "The first real paragraph continues here.",
        )

    def test_first_paragraph_returns_empty_for_code_and_headings_only(self) -> None:
        self.assertEqual(
            markdown_render.first_paragraph(["# Guide", "```text", "example", "```"]),
            "",
        )

    def test_first_paragraph_returns_empty_text_when_there_is_no_prose(self) -> None:
        self.assertEqual(markdown_render.first_paragraph(["# Title", "", "## Subtitle"]), "")

    def test_first_paragraph_skips_fenced_code_before_prose(self) -> None:
        lines = [
            "# Guide",
            "```yaml",
            "setting: value",
            "```",
            "",
            "The actual introduction.",
        ]

        self.assertEqual(markdown_render.first_paragraph(lines), "The actual introduction.")

    def test_first_paragraph_ignores_an_unclosed_fenced_code_block(self) -> None:
        self.assertEqual(markdown_render.first_paragraph(["```text", "not prose"]), "")

    def test_first_paragraph_stops_when_code_follows_prose(self) -> None:
        lines = ["Introductory text.", "```text", "not part of the description", "```"]

        self.assertEqual(markdown_render.first_paragraph(lines), "Introductory text.")

    def test_first_paragraph_stops_when_a_heading_follows_prose(self) -> None:
        lines = ["Opening summary.", "continues here.", "## Details", "Not part of the summary."]

        self.assertEqual(
            markdown_render.first_paragraph(lines),
            "Opening summary. continues here.",
        )


class MarkdownToHtmlTest(unittest.TestCase):
    def test_markdown_to_html_renders_headings_paragraphs_and_code(self) -> None:
        result = markdown_render.markdown_to_html(
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

    def test_markdown_to_html_flushes_a_final_paragraph(self) -> None:
        result = markdown_render.markdown_to_html(["A paragraph", "continued without a blank line."])

        self.assertEqual(result, "<p>A paragraph continued without a blank line.</p>")

    def test_markdown_to_html_rewrites_links_in_headings_and_paragraphs(self) -> None:
        rewritten = []

        def rewrite(href: str) -> str:
            rewritten.append(href)
            return f"published/{href}"

        result = markdown_render.markdown_to_html(
            ["## [Setup](setup.md)", "Read the [guide](guide.md)."], rewrite
        )

        self.assertEqual(rewritten, ["setup.md", "guide.md"])
        self.assertEqual(
            result,
            '<h2><a href="published/setup.md">Setup</a></h2>\n'
            '<p>Read the <a href="published/guide.md">guide</a>.</p>',
        )

    def test_markdown_to_html_accepts_an_unclosed_final_code_fence(self) -> None:
        result = markdown_render.markdown_to_html(["```text", "first", "  second"])

        self.assertEqual(
            result,
            '<pre class="code-block language-text" tabindex="0"><code>first\n  second</code></pre>',
        )

    def test_markdown_to_html_keeps_blank_lines_inside_code_blocks(self) -> None:
        result = markdown_render.markdown_to_html(
            ["Before.", "", "```papyrus", "Function Run()", "", "EndFunction", "```", "After."]
        )

        self.assertIn("Run()\n\n", result)
        self.assertIn('<span class="kw">EndFunction</span>', result)
        self.assertEqual(result.count("<p>"), 2)

    def test_markdown_to_html_escapes_headings_and_code_blocks(self) -> None:
        result = markdown_render.markdown_to_html(
            ["###### <Advanced> & **safe**", "", "```", '<script data-x="1">', "```"]
        )

        self.assertEqual(
            result,
            "<h6>&lt;Advanced&gt; &amp; <strong>safe</strong></h6>\n"
            '<pre class="code-block" tabindex="0"><code>'
            '&lt;script data-x=&quot;1&quot;&gt;</code></pre>',
        )

    def test_markdown_to_html_escapes_an_untrusted_fence_language(self) -> None:
        result = markdown_render.markdown_to_html(
            ['```yaml" onmouseover="alert(1)', "enabled: true", "```"]
        )

        self.assertIn('class="code-block language-yaml&quot;" tabindex="0"', result)
        self.assertNotIn("onmouseover=", result)
        self.assertIn("enabled: true", result)

    def test_markdown_to_html_keeps_backticks_inside_a_code_block_literal(self) -> None:
        result = markdown_render.markdown_to_html(
            ["```shell", "echo ``` is data", "```", "Afterwards"]
        )

        self.assertIn("<code>echo ``` is data</code>", result)
        self.assertTrue(result.endswith("<p>Afterwards</p>"))


if __name__ == "__main__":
    unittest.main()
