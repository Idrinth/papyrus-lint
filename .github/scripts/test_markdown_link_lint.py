#!/usr/bin/env python3
"""Unit tests for the Markdown local-link linter."""

import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import markdown_link_lint


class MarkdownLinkLintTests(unittest.TestCase):
    def test_broken_links_checks_relative_inline_and_reference_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            docs = root / "docs"
            docs.mkdir()
            (root / "README.md").write_text("# Start\n", encoding="utf-8")
            markdown = docs / "guide.md"
            markdown.write_text(
                "[readme](../README.md#start)\n"
                "[missing](missing.md)\n"
                "[encoded](not%20here.md?raw=1#top)\n"
                "[project]: ../README.md\n"
                "[bad]: <also-missing.md>\n",
                encoding="utf-8",
            )

            self.assertEqual(
                [(2, "missing.md"), (3, "not%20here.md?raw=1#top"), (5, "also-missing.md")],
                markdown_link_lint.broken_links(markdown),
            )

    def test_broken_links_checks_github_heading_anchors(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            markdown = root / "README.md"
            guide = root / "guide.md"
            guide.write_text(
                "# Getting Started!\n"
                "## Repeated heading\n"
                "## Repeated heading\n"
                "Linked [`code`](elsewhere.md) &amp; details\n"
                "------------------------------------------\n",
                encoding="utf-8",
            )
            markdown.write_text(
                "# Local Section\n"
                "[local](#local-section) [heading](guide.md#getting-started)\n"
                "[punctuation](guide.md#getting-started) [duplicate](guide.md#repeated-heading-1)\n"
                "[setext](guide.md#linked-code--details)\n"
                "[missing local](#missing) [missing remote](guide.md#missing)\n",
                encoding="utf-8",
            )

            self.assertEqual(
                [(5, "#missing"), (5, "guide.md#missing")],
                markdown_link_lint.broken_links(markdown),
            )

    def test_broken_links_ignores_external_and_code_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            markdown = Path(directory, "README.md")
            markdown.write_text(
                "[web](https://something.de#section) [mail](mailto:hello@example.com)\n"
                "`[inline code](missing.md)`\n"
                "```markdown\n[code fence](missing.md)\n```\n",
                encoding="utf-8",
            )

            self.assertEqual([], markdown_link_lint.broken_links(markdown))

    def test_markdown_files_recurses_and_skips_dependency_directories(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            expected = root / "docs" / "guide.markdown"
            expected.parent.mkdir()
            expected.touch()
            (root / "notes.txt").touch()
            dependency_file = root / "node_modules" / "README.md"
            dependency_file.parent.mkdir()
            dependency_file.touch()

            self.assertEqual([expected], markdown_link_lint.markdown_files(root))

    def test_main_reports_broken_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            markdown = Path(directory, "README.md")
            markdown.write_text("[missing](missing.md)\n", encoding="utf-8")
            output = io.StringIO()
            with (
                mock.patch("sys.argv", ["markdown_link_lint.py", directory]),
                redirect_stdout(output),
            ):
                result = markdown_link_lint.main()

            self.assertEqual(1, result)
            self.assertIn("README.md:1: local link does not exist: missing.md", output.getvalue())
            self.assertTrue(output.getvalue().endswith("Markdown link lint failed with 1 issue(s).\n"))


if __name__ == "__main__":
    unittest.main()
