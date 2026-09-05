"""Tests for the dependency-free GitHub Pages site builder."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

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
                "<main><!--LINT_TABLE:Formatting--><!--CLI_EXAMPLES--><!--DOCS_LIST--><!--VERSION--></main>",
                encoding="utf-8",
            )
            (pages_dir / "docs.template.html").write_text(
                "<!--DOC_TITLE--><!--DOC_DESCRIPTION--><!--DOC_CONTENT-->",
                encoding="utf-8",
            )
            (pages_dir / "styles.css").write_text("main { color: red; }", encoding="utf-8")
            source_asset = root / "source.png"
            source_asset.write_bytes(b"image bytes")
            out_dir = root / "public"
            out_dir.mkdir()
            (out_dir / "stale.txt").write_text("remove me", encoding="utf-8")

            with (
                patch.object(page_builder, "ROOT", root),
                patch.object(page_builder, "PAGES_DIR", pages_dir),
                patch.object(page_builder, "LINT_CATEGORIES", ["Formatting"]),
                patch.object(page_builder, "DOCS", []),
                patch.object(page_builder, "ASSETS", {"copied.png": source_asset}),
                patch.object(page_builder, "DOCS", []),
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
            self.assertEqual(
                (out_dir / "styles.css").read_text(encoding="utf-8"),
                "main { color: red; }",
            )
            self.assertEqual((out_dir / "assets" / "copied.png").read_bytes(), b"image bytes")
            self.assertFalse((out_dir / "stale.txt").exists())
            self.assertTrue((out_dir / "docs" / "index.html").exists())

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


if __name__ == "__main__":
    unittest.main()
