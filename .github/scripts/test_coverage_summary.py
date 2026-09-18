#!/usr/bin/env python3
"""Unit tests for the coverage-by-module summary CLI entrypoint."""

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("coverage_summary.py")
SPEC = importlib.util.spec_from_file_location("coverage_summary", SCRIPT)
assert SPEC and SPEC.loader
coverage_summary = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = coverage_summary
SPEC.loader.exec_module(coverage_summary)


class MainTests(unittest.TestCase):
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
        self.assertIn("| Combined | 75.0% | 3/4 | n/a |", rendered)
        self.assertIn("| ↳ missing | _no report_ | | |", rendered)
        self.assertIn("| Empty | n/a | 0/0 | n/a |", rendered)
        self.assertIn("| **Total** | **75.0%** | **3/4** | **n/a** |", rendered)

    def test_main_uses_na_total_when_every_report_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = io.StringIO()
            with (
                mock.patch.object(coverage_summary, "MODULES", [("Missing", [("part", "none")])]),
                mock.patch.object(sys, "argv", ["coverage_summary.py", directory]),
                contextlib.redirect_stdout(output),
            ):
                coverage_summary.main()

        self.assertIn("| **Total** | **n/a** | **0/0** | **n/a** |", output.getvalue())

    def test_main_shows_part_rows_only_for_composite_modules(self) -> None:
        modules = [
            ("Single", [("single-part", "single.info")]),
            ("Composite", [("first-part", "first.info"), ("second-part", "second.info")]),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for report in ("single.info", "first.info", "second.info"):
                Path(root, report).write_text("LF:2\nLH:1\n", encoding="utf-8")
            output = io.StringIO()

            with (
                mock.patch.object(coverage_summary, "MODULES", modules),
                mock.patch.object(sys, "argv", ["coverage_summary.py", directory]),
                contextlib.redirect_stdout(output),
            ):
                coverage_summary.main()

        rendered = output.getvalue()
        self.assertNotIn("| ↳ single-part |", rendered)
        self.assertIn("| ↳ first-part | 50.0% | 1/2 | n/a |", rendered)
        self.assertIn("| ↳ second-part | 50.0% | 1/2 | n/a |", rendered)
        self.assertIn("| **Total** | **50.0%** | **3/6** | **n/a** |", rendered)


if __name__ == "__main__":
    unittest.main()
