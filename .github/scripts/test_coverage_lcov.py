#!/usr/bin/env python3
"""Unit tests for lcov.info aggregation shared by coverage_summary.py and
render_nexuspage.py."""

import tempfile
import unittest
from pathlib import Path

from ci_lib import coverage_lcov


class CoverageLcovTests(unittest.TestCase):
    def test_modules_group_tooling_coverage(self) -> None:
        self.assertIn(
            (
                "Tooling",
                [
                    ("CI tooling", "ci-scripts-coverage/lcov.info"),
                    ("Pages (site builder)", "pages-coverage/lcov.info"),
                ],
            ),
            coverage_lcov.MODULES,
        )

    def test_parse_lcov_sums_records_and_tolerates_non_utf8_text(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_bytes(
                b"TN:\xff\nLF:10\nLH:8\nFNF:3\nend_of_record\nLF:5\nLH:2\nFNF:1\n"
            )

            self.assertEqual((15, 10, 4), coverage_lcov.parse_lcov(report))

    def test_parse_lcov_ignores_other_numeric_fields(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "TN:unit\nSF:example.py\nFNH:8\nBRF:7\nBRH:6\nLF:5\nLH:4\nFNF:9\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual((5, 4, 9), coverage_lcov.parse_lcov(report))

    def test_parse_lcov_accepts_an_empty_report(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text("", encoding="utf-8")

            self.assertEqual((0, 0, 0), coverage_lcov.parse_lcov(report))

    def test_parse_lcov_returns_none_for_missing_report(self) -> None:
        self.assertIsNone(coverage_lcov.parse_lcov(Path("does-not-exist.info")))

    def test_percentage_handles_empty_and_populated_reports(self) -> None:
        self.assertEqual("n/a", coverage_lcov.pct(0, 0))
        self.assertEqual("62.5%", coverage_lcov.pct(5, 8))

    def test_crap_estimate_combines_average_function_size_and_coverage(self) -> None:
        self.assertAlmostEqual(2.0625, coverage_lcov.crap_estimate(4, 3, 2))
        self.assertAlmostEqual(2.0, coverage_lcov.crap_estimate(2, 2, 1))

    def test_crap_estimate_returns_none_without_enough_data(self) -> None:
        self.assertIsNone(coverage_lcov.crap_estimate(0, 0, 0))
        self.assertIsNone(coverage_lcov.crap_estimate(4, 3, 0))

    def test_format_crap_renders_one_decimal_or_na(self) -> None:
        self.assertEqual("n/a", coverage_lcov.format_crap(None))
        self.assertEqual("2.1", coverage_lcov.format_crap(2.0625))

    def test_iter_leaf_paths_flattens_arbitrarily_nested_groups(self) -> None:
        entries = [
            ("direct", "direct.info"),
            (
                "nested",
                [
                    ("child", "child.info"),
                    ("deeper", [("grandchild", "grandchild.info")]),
                ],
            ),
        ]

        self.assertEqual(
            ["direct.info", "child.info", "grandchild.info"],
            list(coverage_lcov.iter_leaf_paths(entries)),
        )

    def test_render_entry_includes_nested_group_totals_and_leaf_rows(self) -> None:
        value = [
            ("first", "first.info"),
            ("nested", [("second", "second.info"), ("missing", "missing.info")]),
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "first.info").write_text("LF:4\nLH:3\nFNF:2\n", encoding="utf-8")
            (root / "second.info").write_text("LF:6\nLH:2\nFNF:3\n", encoding="utf-8")

            found, hit, functions_found, rows = coverage_lcov.render_entry(root, "Group", value, 0)

        self.assertEqual((10, 5, 5), (found, hit, functions_found))
        self.assertEqual(
            [
                "| Group | 50.0% | 5/10 | 2.5 |",
                "| ↳ first | 75.0% | 3/4 | 2.1 |",
                "| ↳ nested | 33.3% | 2/6 | 3.2 |",
                "| ↳ ↳ second | 33.3% | 2/6 | 3.2 |",
                "| ↳ ↳ missing | _no report_ | | |",
            ],
            rows,
        )

    def test_render_entry_hides_the_only_child_of_a_group(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "only.info").write_text("LF:2\nLH:2\nFNF:1\n", encoding="utf-8")

            result = coverage_lcov.render_entry(
                root, "Wrapper", [("only", "only.info")], 1
            )

        self.assertEqual((2, 2, 1, ["| ↳ Wrapper | 100.0% | 2/2 | 2.0 |"]), result)


if __name__ == "__main__":
    unittest.main()
