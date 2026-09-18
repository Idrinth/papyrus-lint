"""Tests for screenshot/logo/schema asset emission."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image

from pages import site_assets


class ConvertToModernFormatsTest(unittest.TestCase):
    def test_writes_a_lossless_webp_and_avif_sibling_for_a_png(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            directory_path = Path(directory)
            source = directory_path / "screenshot.png"
            Image.new("RGB", (4, 4), (10, 20, 30)).save(source)

            site_assets.convert_to_modern_formats(source, directory_path)

            self.assertTrue((directory_path / "screenshot.webp").is_file())
            self.assertTrue((directory_path / "screenshot.avif").is_file())

    def test_writes_a_lossy_webp_and_avif_sibling_for_a_jpeg_and_converts_to_rgb(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            directory_path = Path(directory)
            source = directory_path / "logo.jpg"
            Image.new("CMYK", (4, 4), (0, 0, 0, 0)).save(source)

            site_assets.convert_to_modern_formats(source, directory_path)

            self.assertTrue((directory_path / "logo.webp").is_file())
            self.assertTrue((directory_path / "logo.avif").is_file())


class WrapImagesWithModernSourcesTest(unittest.TestCase):
    def test_wraps_a_modern_format_asset_image_in_a_picture_element(self) -> None:
        with patch.object(site_assets, "MODERN_FORMAT_ASSETS", {"screenshot.png"}):
            result = site_assets.wrap_images_with_modern_sources(
                '<img src="assets/screenshot.png" alt="Screenshot" />'
            )

        self.assertEqual(
            result,
            "<picture>"
            '<source srcset="assets/screenshot.avif" type="image/avif" />'
            '<source srcset="assets/screenshot.webp" type="image/webp" />'
            '<img src="assets/screenshot.png" alt="Screenshot" />'
            "</picture>",
        )

    def test_wraps_a_docs_subpage_relative_image_preserving_the_prefix(self) -> None:
        with patch.object(site_assets, "MODERN_FORMAT_ASSETS", {"screenshot.png"}):
            result = site_assets.wrap_images_with_modern_sources(
                '<img src="../assets/screenshot.png" alt="Screenshot" />'
            )

        self.assertIn('<source srcset="../assets/screenshot.avif" type="image/avif" />', result)
        self.assertIn('<source srcset="../assets/screenshot.webp" type="image/webp" />', result)

    def test_leaves_a_non_modern_format_asset_image_untouched(self) -> None:
        with patch.object(site_assets, "MODERN_FORMAT_ASSETS", {"screenshot.png"}):
            html_in = '<img src="assets/logo.jpg" alt="Logo" />'
            result = site_assets.wrap_images_with_modern_sources(html_in)

        self.assertEqual(result, html_in)
        self.assertNotIn("<picture>", result)

    def test_leaves_a_remote_image_untouched(self) -> None:
        html_in = '<img src="https://example.test/badge.png" alt="Badge" />'
        result = site_assets.wrap_images_with_modern_sources(html_in)

        self.assertEqual(result, html_in)


class RepositoryAssetConfigurationTest(unittest.TestCase):
    """Keep site_assets.py's checked-in inputs synchronized with the repository."""

    def test_copy_json_schemas_publishes_only_schemata_unchanged(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            schema_dir = root / "schema"
            schema_dir.mkdir()
            (schema_dir / "first.schema.json").write_bytes(b'{"title": "First"}\n')
            (schema_dir / "second.schema.json").write_bytes(b'{\n  "type": "object"\n}\n')
            (schema_dir / site_assets.AI_EXPORT_V1_SCHEMA).write_bytes(b'{"title": "AI export v1"}\n')
            (schema_dir / "ordinary.json").write_bytes(b"{}\n")
            out_dir = root / "site"
            out_dir.mkdir()

            with patch.object(site_assets, "SCHEMA_DIR", schema_dir):
                site_assets.copy_json_schemas(out_dir)

            self.assertEqual(
                (out_dir / "schema" / "first.schema.json").read_bytes(),
                b'{"title": "First"}\n',
            )
            self.assertEqual(
                (out_dir / "schema" / "second.schema.json").read_bytes(),
                b'{\n  "type": "object"\n}\n',
            )
            self.assertEqual(
                (out_dir / "schema" / site_assets.AI_EXPORT_LEGACY_SCHEMA).read_bytes(),
                b'{"title": "AI export v1"}\n',
            )
            self.assertFalse((out_dir / "schema" / "ordinary.json").exists())

    def test_asset_manifest_points_to_files_and_modern_assets_are_a_subset(self) -> None:
        self.assertLessEqual(site_assets.MODERN_FORMAT_ASSETS, site_assets.ASSETS.keys())
        for output_name, source in site_assets.ASSETS.items():
            with self.subTest(asset=output_name):
                self.assertTrue(source.is_file(), f"missing site asset: {source}")
                self.assertEqual(Path(output_name).suffix.lower(), source.suffix.lower())


if __name__ == "__main__":
    unittest.main()
