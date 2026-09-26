"""Tests for static site pages."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import site_chrome, static_pages


class ImprintPageTest(unittest.TestCase):
    def test_build_imprint_page_runs_the_rendering_pipeline_with_default_version(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "imprint.template.html").write_text("<main>Imprint template</main>", encoding="utf-8")

            with (
                patch.object(static_pages, "PAGES_DIR", pages_dir),
                patch.object(
                    static_pages,
                    "render_shared_components",
                    return_value="<main>Rendered imprint</main>",
                ) as render_shared_components,
                patch.object(
                    static_pages,
                    "finalize_page",
                    return_value="<main>Finalisé</main>",
                ) as finalize_page,
            ):
                static_pages.build_imprint_page(out_dir)

            output = (out_dir / "imprint.html").read_text(encoding="utf-8")

        render_shared_components.assert_called_once_with("<main>Imprint template</main>", "", "")
        finalize_page.assert_called_once_with("<main>Rendered imprint</main>")
        self.assertEqual(output, "<main>Finalisé</main>")

    def test_build_imprint_page_applies_shared_chrome(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            includes_dir = pages_dir / "includes"
            out_dir = root / "out"
            includes_dir.mkdir(parents=True)
            out_dir.mkdir()
            (pages_dir / "imprint.template.html").write_text(
                "<!--SITE_HEADER--><main>Legal Notice</main><!--SITE_FOOTER-->", encoding="utf-8"
            )
            (includes_dir / "header.html").write_text("<header>Imprint</header>", encoding="utf-8")
            (includes_dir / "footer.html").write_text("<footer><!--VERSION--></footer>", encoding="utf-8")

            with (
                patch.object(static_pages, "PAGES_DIR", pages_dir),
                patch.object(site_chrome, "INCLUDES_DIR", includes_dir),
                patch.object(site_chrome, "render_funding_links", return_value=""),
            ):
                static_pages.build_imprint_page(out_dir, 'v2<&"')

            output = (out_dir / "imprint.html").read_text(encoding="utf-8")

        self.assertIn("<header>Imprint</header>", output)
        self.assertIn("Legal Notice", output)
        self.assertIn("<footer>v2&lt;&amp;&quot;</footer>", output)

    def test_build_imprint_page_does_not_create_output_when_template_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()

            with (
                patch.object(static_pages, "PAGES_DIR", pages_dir),
                patch.object(static_pages, "render_shared_components") as render_shared_components,
                patch.object(static_pages, "finalize_page") as finalize_page,
                self.assertRaises(FileNotFoundError),
            ):
                static_pages.build_imprint_page(out_dir, "v3")

            render_shared_components.assert_not_called()
            finalize_page.assert_not_called()
            self.assertFalse((out_dir / "imprint.html").exists())
