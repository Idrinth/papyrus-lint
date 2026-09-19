#!/usr/bin/env python3
"""Unit tests for the Nexus page coverage-marker rendering logic."""

import tempfile
import unittest
from pathlib import Path
from unittest import mock

from ci_lib import nexuspage_render


class CoverageTotalsTests(unittest.TestCase):
    def test_coverage_totals_aggregates_every_report(self) -> None:
        modules = [("Module", [("one", "one.info"), ("two", "nested/two.info")])]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Path(root, "one.info").write_text("LF:3\nLH:2\n", encoding="utf-8")
            Path(root, "nested").mkdir()
            Path(root, "nested/two.info").write_text("LF:7\nLH:4\n", encoding="utf-8")

            with mock.patch.object(nexuspage_render, "MODULES", modules):
                self.assertEqual((6, 10), nexuspage_render.coverage_totals(root))

    def test_coverage_totals_requires_all_reports(self) -> None:
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(
            nexuspage_render, "MODULES", [("Module", [("one", "missing.info")])]
        ), self.assertRaisesRegex(ValueError, "missing coverage reports: missing.info"):
            nexuspage_render.coverage_totals(Path(directory))

    def test_coverage_totals_rejects_reports_without_lines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "empty.info").write_text("LF:0\nLH:0\n", encoding="utf-8")
            with mock.patch.object(
                nexuspage_render, "MODULES", [("Module", [("empty", "empty.info")])]
            ), self.assertRaisesRegex(ValueError, "coverage reports contain no lines"):
                nexuspage_render.coverage_totals(Path(directory))

    def test_coverage_totals_lists_every_missing_nested_report(self) -> None:
        modules = [
            (
                "Module",
                [
                    ("nested", [("one", "one.info"), ("two", "two.info")]),
                    ("three", "three.info"),
                ],
            )
        ]
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(
            nexuspage_render, "MODULES", modules
        ), self.assertRaisesRegex(
            ValueError, "missing coverage reports: one.info, two.info, three.info"
        ):
            nexuspage_render.coverage_totals(Path(directory))

    def test_coverage_totals_reports_missing_files_before_empty_coverage(self) -> None:
        modules = [
            (
                "Module",
                [("empty", "empty.info"), ("missing", "missing.info")],
            )
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "empty.info").write_text("LF:0\nLH:0\n", encoding="utf-8")

            with (
                mock.patch.object(nexuspage_render, "MODULES", modules),
                self.assertRaisesRegex(
                    ValueError, "missing coverage reports: missing.info"
                ),
            ):
                nexuspage_render.coverage_totals(root)

    def test_coverage_totals_supports_deeply_nested_module_groups(self) -> None:
        modules = [
            (
                "Module",
                [
                    (
                        "group",
                        [("subgroup", [("report", "nested/report.info")])],
                    )
                ],
            )
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Path(root, "nested").mkdir()
            Path(root, "nested/report.info").write_text(
                "LF:12\nLH:9\n", encoding="utf-8"
            )

            with mock.patch.object(nexuspage_render, "MODULES", modules):
                self.assertEqual((9, 12), nexuspage_render.coverage_totals(root))


class RenderTests(unittest.TestCase):
    def test_render_replaces_counts_percentage_and_version(self) -> None:
        template = "<COVERED_LINES> / <TOTAL_LINES> (~<COVERAGE_PERCENTAGE>%) <VERSION>"
        self.assertEqual(
            "7 / 8 (~87.5%) v1.2.3", nexuspage_render.render(template, 7, 8, "v1.2.3")
        )

    def test_render_formats_large_counts_with_thousands_separators(self) -> None:
        template = "<COVERED_LINES> / <TOTAL_LINES> (~<COVERAGE_PERCENTAGE>%) <VERSION>"
        self.assertEqual(
            "27,484 / 27,965 (~98.3%) v1.2.3",
            nexuspage_render.render(template, 27_484, 27_965, "v1.2.3"),
        )

    def test_render_handles_zero_covered_lines(self) -> None:
        template = "<COVERED_LINES>/<TOTAL_LINES> (<COVERAGE_PERCENTAGE>%) <VERSION>"

        self.assertEqual(
            "0/25 (0.0%) v2.0.0",
            nexuspage_render.render(template, 0, 25, "v2.0.0"),
        )

    def test_render_preserves_unicode_and_newlines_around_markers(self) -> None:
        template = (
            "[heading]Übersicht[/heading]\n"
            "<COVERED_LINES> von <TOTAL_LINES> · <COVERAGE_PERCENTAGE>%\n"
            "Version <VERSION> — fertig\n"
        )

        self.assertEqual(
            "[heading]Übersicht[/heading]\n3 von 4 · 75.0%\nVersion v1.0.0 — fertig\n",
            nexuspage_render.render(template, 3, 4, "v1.0.0"),
        )

    def test_render_fills_links_marker_from_shared_yaml(self) -> None:
        result = nexuspage_render.render("go <LINKS> here", 1, 2, "v1")

        self.assertNotIn("<LINKS>", result)
        self.assertIn("[list]", result)
        self.assertIn("[url=https://discord.gg/idrinth]Discord[/url]", result)
        self.assertIn(
            "[url=https://marketplace.visualstudio.com/items?itemName=Idrinth.papyrus-lint-vscode]"
            "VSCode Extension[/url]",
            result,
        )

    def test_render_filters_contact_links_by_the_marker_tag(self) -> None:
        result = nexuspage_render.render("<CONTACT-LINKS>", 1, 2, "v1")

        self.assertIn("[url=https://discord.gg/idrinth]Discord[/url]", result)
        self.assertIn("[url=https://tally.so/r/aQL1dB]Tally Feedback Form[/url]", result)
        self.assertNotIn("marketplace.visualstudio.com", result)


if __name__ == "__main__":
    unittest.main()
