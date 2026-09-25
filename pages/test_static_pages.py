"""Tests for static site pages."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import site_chrome, static_pages


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
                "<!--SITE_HEADER--><main>Legal Notice</main><!--SITE_FOOTER-->", encoding="utf-8"
            )
            (includes_dir / "header.html").write_text("<header>Imprint</header>", encoding="utf-8")
            (includes_dir / "footer.html").write_text(
                "<footer><!--VERSION--></footer>", encoding="utf-8"
            )

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
