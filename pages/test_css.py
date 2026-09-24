"""Tests for CSS `@import` inlining."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from pages import css

PAGES_DIR = Path(__file__).resolve().parent


class InlineCssImportsTest(unittest.TestCase):
    def test_site_entrypoint_inlines_all_style_modules(self) -> None:
        entry = PAGES_DIR / "styles.css"

        result = css.inline_css_imports(entry.read_text(encoding="utf-8"), entry)

        self.assertNotIn("@import", result)
        for selector in (
            ":root",
            ".site-header",
            "pre.code-block",
            ".hero",
            ".doc-content",
            ".coverage-content",
            ".rules-content",
            "@media print",
        ):
            with self.subTest(selector=selector):
                self.assertIn(selector, result)

    def test_inlines_a_relative_import(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            directory_path = Path(directory)
            (directory_path / "shared.css").write_text("body { color: red; }\n", encoding="utf-8")
            entry = directory_path / "styles.css"
            entry.write_text('@import "shared.css";\nmain { color: blue; }\n', encoding="utf-8")

            result = css.inline_css_imports(entry.read_text(encoding="utf-8"), entry)

        self.assertEqual(result, "body { color: red; }\n\nmain { color: blue; }\n")
        self.assertNotIn("@import", result)

    def test_inlines_a_url_form_import_and_chases_nested_imports(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            directory_path = Path(directory)
            (directory_path / "base.css").write_text(".base {}\n", encoding="utf-8")
            (directory_path / "shared.css").write_text('@import url("base.css");\n.shared {}\n', encoding="utf-8")
            entry = directory_path / "styles.css"
            entry.write_text("@import url('shared.css');\n.entry {}\n", encoding="utf-8")

            result = css.inline_css_imports(entry.read_text(encoding="utf-8"), entry)

        self.assertEqual(result, ".base {}\n\n.shared {}\n\n.entry {}\n")

    def test_leaves_remote_imports_untouched(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            entry = Path(directory) / "styles.css"
            entry.write_text(
                '@import "https://example.test/a.css";\n'
                "@import url('http://example.test/b.css');\n"
                '@import "//example.test/c.css";\n',
                encoding="utf-8",
            )

            result = css.inline_css_imports(entry.read_text(encoding="utf-8"), entry)

        self.assertIn('@import "https://example.test/a.css";', result)
        self.assertIn("@import url('http://example.test/b.css');", result)
        self.assertIn('@import "//example.test/c.css";', result)

    def test_rejects_a_missing_import(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            entry = Path(directory) / "styles.css"
            entry.write_text('@import "missing.css";\n', encoding="utf-8")

            with self.assertRaisesRegex(SystemExit, "@import not found: missing.css"):
                css.inline_css_imports(entry.read_text(encoding="utf-8"), entry)

    def test_rejects_a_cyclical_import(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            directory_path = Path(directory)
            entry = directory_path / "a.css"
            other = directory_path / "b.css"
            entry.write_text('@import "b.css";\n', encoding="utf-8")
            other.write_text('@import "a.css";\n', encoding="utf-8")

            with self.assertRaisesRegex(SystemExit, "cyclical @import"):
                css.inline_css_imports(entry.read_text(encoding="utf-8"), entry)


if __name__ == "__main__":
    unittest.main()
