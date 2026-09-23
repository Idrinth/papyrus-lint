"""Tests for the Pages asset minifiers."""

from __future__ import annotations

import unittest

from pages.minify import minify_css, minify_html, minify_js


class MinifyHtmlTest(unittest.TestCase):
    def test_removes_comments_and_collapses_regular_markup(self) -> None:
        result = minify_html(
            """<!doctype html>
            <!-- deployment note -->
            <html>
              <body>
                <p>Hello</p>
                <p>world</p>
              </body>
            </html>
            """
        )

        self.assertNotIn("deployment note", result)
        self.assertNotIn("\n", result)
        self.assertIn("<p>Hello</p><p>world</p>", result)

    def test_preserves_pre_blocks_verbatim(self) -> None:
        pre = '<PRE class="sample">first line\n  <!-- literal example -->\nthird line</PRE>'

        result = minify_html(f"<main>  before  {pre}  after  </main>")

        self.assertIn(pre, result)
        self.assertIn("<!-- literal example -->", result)

    def test_preserves_multiple_pre_blocks_in_their_original_order(self) -> None:
        first = "<pre>one\n  two</pre>"
        second = '<pre data-language="papyrus">three\n    four</pre>'

        result = minify_html(f"<article>{first}<p>middle</p>{second}</article>")

        self.assertIn(first, result)
        self.assertIn(second, result)
        self.assertLess(result.index(first), result.index(second))

    def test_normalizes_self_closing_link_and_meta_tags_only(self) -> None:
        result = minify_html(
            '<head><meta charset="utf-8" /><link rel="stylesheet" href="site.css" /></head>'
            '<body><svg><path d="M0 0" /></svg></body>'
        )

        head = result[result.index("<head>") : result.index("</head>")]
        self.assertIn("<meta charset=utf-8>", head)
        self.assertIn("<link rel=stylesheet href=site.css>", head)
        self.assertNotIn("/>", head)
        self.assertIn('<path d="M0 0"/>', result)

    def test_returns_an_empty_string_for_whitespace_only_input(self) -> None:
        self.assertEqual(minify_html(" \n\t "), "")


class MinifyCssTest(unittest.TestCase):
    def test_removes_comments_and_unnecessary_whitespace(self) -> None:
        result = minify_css("/* note */\n.card { color: red; margin: 0 1rem; }\n")

        self.assertEqual(result, ".card{color:red;margin:0 1rem}")

    def test_preserves_content_string_whitespace(self) -> None:
        result = minify_css('.label::after { content: "two  spaces"; }')

        self.assertIn('content:"two  spaces"', result)


class MinifyJavaScriptTest(unittest.TestCase):
    def test_removes_comments_and_compacts_javascript(self) -> None:
        result = minify_js(
            """// setup
            function add(left, right) {
                return left + right;
            }
            """
        )

        self.assertEqual(result, "function add(left,right){return left+right;}")

    def test_preserves_comment_like_text_inside_strings(self) -> None:
        result = minify_js('const url = "https://example.test/a//b";')

        self.assertIn('"https://example.test/a//b"', result)


if __name__ == "__main__":
    unittest.main()
