#!/usr/bin/env python3
"""Unit tests for the coverage summary and Nexus page rendering scripts."""

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


def load_script(name: str):
    path = Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


coverage_summary = load_script("coverage_summary")
render_nexuspage = load_script("render_nexuspage")


class CoverageSummaryTests(unittest.TestCase):
    def test_modules_include_ci_scripts_coverage(self) -> None:
        self.assertIn(
            ("CI tooling", [(".github/scripts", "ci-scripts-coverage/lcov.info")]),
            coverage_summary.MODULES,
        )

    def test_modules_include_pages_builder_coverage(self) -> None:
        self.assertIn(
            ("Pages (site builder)", [("pages", "pages-coverage/lcov.info")]),
            coverage_summary.MODULES,
        )

    def test_parse_lcov_sums_records_and_tolerates_non_utf8_text(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_bytes(b"TN:\xff\nLF:10\nLH:8\nend_of_record\nLF:5\nLH:2\n")

            self.assertEqual((15, 10), coverage_summary.parse_lcov(report))

    def test_parse_lcov_returns_none_for_missing_report(self) -> None:
        self.assertIsNone(coverage_summary.parse_lcov(Path("does-not-exist.info")))

    def test_percentage_handles_empty_and_populated_reports(self) -> None:
        self.assertEqual("n/a", coverage_summary.pct(0, 0))
        self.assertEqual("62.5%", coverage_summary.pct(5, 8))

    def test_main_reports_module_parts_missing_reports_and_total(self) -> None:
        modules = [
            ("Combined", [("first", "first/lcov.info"), ("missing", "missing/lcov.info")]),
            ("Empty", [("zero", "zero/lcov.info")]),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Path(root, "first").mkdir()
            Path(root, "first/lcov.info").write_text("LF:4\nLH:3\n", encoding="utf-8")
            Path(root, "zero").mkdir()
            Path(root, "zero/lcov.info").write_text("LF:0\nLH:0\n", encoding="utf-8")
            output = io.StringIO()

            with (
                mock.patch.object(coverage_summary, "MODULES", modules),
                mock.patch.object(sys, "argv", ["coverage_summary.py", directory]),
                contextlib.redirect_stdout(output),
            ):
                coverage_summary.main()

        rendered = output.getvalue()
        self.assertIn(coverage_summary.MARKER, rendered)
        self.assertIn("| Combined | 75.0% | 3/4 |", rendered)
        self.assertIn("| ↳ missing | _no report_ | |", rendered)
        self.assertIn("| Empty | n/a | 0/0 |", rendered)
        self.assertIn("| **Total** | **75.0%** | **3/4** |", rendered)

    def test_main_uses_na_total_when_every_report_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = io.StringIO()
            with (
                mock.patch.object(coverage_summary, "MODULES", [("Missing", [("part", "none")])]),
                mock.patch.object(sys, "argv", ["coverage_summary.py", directory]),
                contextlib.redirect_stdout(output),
            ):
                coverage_summary.main()

        self.assertIn("| **Total** | **n/a** | **0/0** |", output.getvalue())


class RenderNexusPageTests(unittest.TestCase):
    def test_coverage_totals_aggregates_every_report(self) -> None:
        modules = [("Module", [("one", "one.info"), ("two", "nested/two.info")])]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            Path(root, "one.info").write_text("LF:3\nLH:2\n", encoding="utf-8")
            Path(root, "nested").mkdir()
            Path(root, "nested/two.info").write_text("LF:7\nLH:4\n", encoding="utf-8")

            with mock.patch.object(render_nexuspage, "MODULES", modules):
                self.assertEqual((6, 10), render_nexuspage.coverage_totals(root))

    def test_coverage_totals_requires_all_reports(self) -> None:
        with tempfile.TemporaryDirectory() as directory, mock.patch.object(
            render_nexuspage, "MODULES", [("Module", [("one", "missing.info")])]
        ):
            with self.assertRaisesRegex(ValueError, "missing coverage reports: missing.info"):
                render_nexuspage.coverage_totals(Path(directory))

    def test_coverage_totals_rejects_reports_without_lines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "empty.info").write_text("LF:0\nLH:0\n", encoding="utf-8")
            with mock.patch.object(
                render_nexuspage, "MODULES", [("Module", [("empty", "empty.info")])]
            ):
                with self.assertRaisesRegex(ValueError, "coverage reports contain no lines"):
                    render_nexuspage.coverage_totals(Path(directory))

    def test_render_replaces_counts_and_percentage(self) -> None:
        template = "<COVERED_LINES> / <TOTAL_LINES> (~<COVERAGE_PERCENTAGE>%)"
        self.assertEqual("7 / 8 (~87.5%)", render_nexuspage.render(template, 7, 8))

    def test_render_requires_every_marker_exactly_once(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected exactly one <COVERED_LINES> marker, found 0"):
            render_nexuspage.render("<TOTAL_LINES> <COVERAGE_PERCENTAGE>", 1, 2)

        template = "<COVERED_LINES> <COVERED_LINES> <TOTAL_LINES> <COVERAGE_PERCENTAGE>"
        with self.assertRaisesRegex(ValueError, "expected exactly one <COVERED_LINES> marker, found 2"):
            render_nexuspage.render(template, 1, 2)

    def test_main_renders_template_to_output_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            template = root / "template.bbcode"
            output = root / "output.bbcode"
            template.write_text(
                "<COVERED_LINES>/<TOTAL_LINES> (<COVERAGE_PERCENTAGE>)", encoding="utf-8"
            )
            with (
                mock.patch.object(sys, "argv", ["render_nexuspage.py", str(template), str(root), str(output)]),
                mock.patch.object(render_nexuspage, "coverage_totals", return_value=(9, 10)),
            ):
                render_nexuspage.main()

            self.assertEqual("9/10 (90.0)", output.read_text(encoding="utf-8"))

    def test_main_rejects_invalid_argument_count(self) -> None:
        with mock.patch.object(sys, "argv", ["render_nexuspage.py"]):
            with self.assertRaisesRegex(SystemExit, "usage: render_nexuspage.py"):
                render_nexuspage.main()


if __name__ == "__main__":
    unittest.main()
