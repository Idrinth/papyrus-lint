"""Tests for coverage report parsing and formatting."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import coverage_report


class CoverageReportTest(unittest.TestCase):
    def test_load_coverage_summary_loads_the_shared_ci_module(self) -> None:
        coverage_summary = coverage_report.load_coverage_summary()

        self.assertEqual(coverage_summary.pct(7, 8), "87.5%")
        self.assertIn(
            ("Pages (site builder)", "pages-coverage/lcov.info"),
            next(parts for label, parts in coverage_summary.MODULES if label == "Tooling"),
        )

    def test_normalize_source_path_strips_a_ci_checkout_prefix(self) -> None:
        self.assertEqual(
            coverage_report.normalize_source_path(
                "/home/runner/work/papyrus-lint/papyrus-lint/app/crates/papyrus-parser/src/lexer.rs"
            ),
            "app/crates/papyrus-parser/src/lexer.rs",
        )

    def test_normalize_source_path_leaves_an_already_relative_path_unchanged(self) -> None:
        self.assertEqual(coverage_report.normalize_source_path("src/main.ts"), "src/main.ts")

    def test_normalize_source_path_normalizes_windows_style_separators(self) -> None:
        self.assertEqual(
            coverage_report.normalize_source_path(
                r"C:\work\papyrus-lint\papyrus-lint\app\crates\papyrus-lints\src\lib.rs"
            ),
            "app/crates/papyrus-lints/src/lib.rs",
        )

    def test_normalize_source_path_uses_the_last_checkout_marker(self) -> None:
        result = coverage_report.normalize_source_path(
            "/cache/papyrus-lint/archive/papyrus-lint/pages/build.py"
        )

        self.assertEqual(result, "pages/build.py")

    def test_normalize_source_path_strips_surrounding_whitespace_before_matching(self) -> None:
        result = coverage_report.normalize_source_path(
            "  /home/runner/work/papyrus-lint/papyrus-lint/pages/build.py  "
        )

        self.assertEqual(result, "pages/build.py")

    def test_parse_lcov_files_returns_none_for_a_missing_report(self) -> None:
        self.assertIsNone(coverage_report.parse_lcov_files(Path("does-not-exist.info")))

    def test_parse_lcov_files_returns_per_file_records(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:/home/runner/work/papyrus-lint/papyrus-lint/app/src/one.rs\n"
                "LF:10\nLH:8\nend_of_record\n"
                "SF:app/src/two.rs\n"
                "LF:4\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual(
                coverage_report.parse_lcov_files(report),
                [("app/src/one.rs", 10, 8), ("app/src/two.rs", 4, 1)],
            )

    def test_parse_lcov_files_ignores_a_record_without_a_source_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text("LF:5\nLH:5\nend_of_record\n", encoding="utf-8")

            self.assertEqual(coverage_report.parse_lcov_files(report), [])

    def test_parse_lcov_files_accumulates_repeated_summary_lines(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:src/generated.rs\nLF:3\nLF:2\nLH:1\nLH:2\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual(
                coverage_report.parse_lcov_files(report),
                [("src/generated.rs", 5, 3)],
            )

    def test_parse_lcov_files_resets_counts_when_a_new_record_starts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:incomplete.rs\nLF:50\nLH:40\n"
                "SF:complete.rs\nLF:4\nLH:3\nend_of_record\n",
                encoding="utf-8",
            )

            self.assertEqual(
                coverage_report.parse_lcov_files(report),
                [("complete.rs", 4, 3)],
            )

    def test_parse_lcov_files_discards_an_unterminated_record(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:complete.rs\nLF:2\nLH:2\nend_of_record\n"
                "SF:partial.rs\nLF:10\nLH:9\n",
                encoding="utf-8",
            )

            result = coverage_report.parse_lcov_files(report)

        self.assertEqual(result, [("complete.rs", 2, 2)])

    def test_parse_lcov_files_replaces_invalid_utf8_in_source_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_bytes(b"SF:src/invalid-\xff.rs\nLF:1\nLH:1\nend_of_record\n")

            result = coverage_report.parse_lcov_files(report)

        self.assertEqual(result, [("src/invalid-\ufffd.rs", 1, 1)])

    def test_parse_lcov_files_handles_empty_and_zero_coverage_records(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "TN:site-builder\n"
                "SF:pages/empty.py\nend_of_record\n"
                "SF:pages/untested.py\nLF:3\nLH:0\nBRF:2\nBRH:0\nend_of_record\n",
                encoding="utf-8",
            )

            result = coverage_report.parse_lcov_files(report)

        self.assertEqual(
            result,
            [("pages/empty.py", 0, 0), ("pages/untested.py", 3, 0)],
        )

    def test_parse_lcov_files_ignores_an_unterminated_record(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:pages/complete.py\nLF:4\nLH:3\nend_of_record\n"
                "SF:pages/incomplete.py\nLF:10\nLH:9\n",
                encoding="utf-8",
            )

            result = coverage_report.parse_lcov_files(report)

        self.assertEqual(result, [("pages/complete.py", 4, 3)])

    def test_parse_lcov_files_adds_repeated_line_summaries_within_a_record(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report = Path(directory, "lcov.info")
            report.write_text(
                "SF:pages/build.py\nLF:3\nLH:2\nLF:4\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )

            result = coverage_report.parse_lcov_files(report)

        self.assertEqual(result, [("pages/build.py", 7, 3)])

    def test_render_coverage_table_renders_rows_with_percentage_and_counts(self) -> None:
        coverage_summary = coverage_report.load_coverage_summary()
        result = coverage_report.render_coverage_table([("src/one.rs", 10, 8)], coverage_summary)

        self.assertIn("<code>src/one.rs</code>", result)
        self.assertIn("<td>80.0%</td>", result)
        self.assertIn("<td>8/10</td>", result)

    def test_render_coverage_table_handles_no_rows(self) -> None:
        coverage_summary = coverage_report.load_coverage_summary()
        result = coverage_report.render_coverage_table([], coverage_summary)

        self.assertEqual(result, '<p class="section-intro">No files reported.</p>')

    def test_render_coverage_table_escapes_source_file_names(self) -> None:
        coverage_summary = coverage_report.load_coverage_summary()

        result = coverage_report.render_coverage_table(
            [('src/<unsafe>&"file".rs', 2, 1)], coverage_summary
        )

        self.assertIn("<code>src/&lt;unsafe&gt;&amp;&quot;file&quot;.rs</code>", result)
        self.assertNotIn("<unsafe>", result)

    def test_render_coverage_table_preserves_the_supplied_row_order(self) -> None:
        coverage_summary = coverage_report.load_coverage_summary()

        result = coverage_report.render_coverage_table(
            [("lowest.rs", 10, 1), ("middle.rs", 10, 5), ("highest.rs", 10, 9)],
            coverage_summary,
        )

        self.assertLess(result.index("lowest.rs"), result.index("middle.rs"))
        self.assertLess(result.index("middle.rs"), result.index("highest.rs"))

    def test_build_coverage_content_groups_by_module_and_sorts_worst_first(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            coverage_summary = coverage_report.load_coverage_summary()
            coverage_summary.MODULES = [
                (
                    "Combined",
                    [
                        ("first-part", "first/lcov.info"),
                        ("second-part", "second/lcov.info"),
                    ],
                ),
                ("Missing", [("absent", "absent/lcov.info")]),
            ]
            first_dir = root / "first"
            first_dir.mkdir()
            (first_dir / "lcov.info").write_text(
                "SF:good.rs\nLF:10\nLH:10\nend_of_record\n"
                "SF:bad.rs\nLF:10\nLH:2\nend_of_record\n",
                encoding="utf-8",
            )
            second_dir = root / "second"
            second_dir.mkdir()
            (second_dir / "lcov.info").write_text("SF:only.rs\nLF:4\nLH:4\nend_of_record\n", encoding="utf-8")

            result = coverage_report.build_coverage_content(root, coverage_summary)

        self.assertIn("Combined", result)
        self.assertIn("Missing", result)
        self.assertIn("No report.", result)
        self.assertLess(result.index("bad.rs"), result.index("good.rs"))
        self.assertIn("Total line coverage: <strong>66.7%</strong> (16/24)", result)

    def test_build_coverage_content_reports_na_when_nothing_is_available(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            coverage_summary = coverage_report.load_coverage_summary()
            coverage_summary.MODULES = [("Missing", [("absent", "absent/lcov.info")])]

            result = coverage_report.build_coverage_content(root, coverage_summary)

        self.assertIn("Total line coverage: <strong>n/a</strong> (0/0)", result)

    def test_build_coverage_content_treats_an_empty_report_as_available(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report_dir = root / "empty"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text("", encoding="utf-8")
            coverage_summary = coverage_report.load_coverage_summary()
            coverage_summary.MODULES = [("Tooling", [("empty report", "empty/lcov.info")])]

            result = coverage_report.build_coverage_content(root, coverage_summary)

        self.assertIn("Total line coverage: <strong>n/a</strong> (0/0)", result)
        self.assertIn("No files reported.", result)
        self.assertNotIn("No report.", result)

    def test_render_coverage_entry_recurses_through_nested_groups(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report_dir = root / "child"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text(
                "SF:empty.rs\nLF:0\nLH:0\nend_of_record\n",
                encoding="utf-8",
            )
            coverage_summary = coverage_report.load_coverage_summary()

            found, hit, any_report, result = coverage_report.render_coverage_entry(
                root,
                "parent",
                [("available", "child/lcov.info"), ("missing", "missing/lcov.info")],
                coverage_summary,
            )

        self.assertEqual((found, hit, any_report), (0, 0, True))
        self.assertIn("<h3>available — n/a (0/0)</h3>", result)
        self.assertIn("<code>empty.rs</code>", result)
        self.assertIn("<h3>missing — n/a (0/0)</h3>", result)
        self.assertIn("No report.", result)

    def test_render_coverage_entry_omits_a_redundant_heading_for_one_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report_dir = root / "only"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text(
                "SF:src/only.py\nLF:4\nLH:3\nend_of_record\n",
                encoding="utf-8",
            )
            coverage_summary = coverage_report.load_coverage_summary()

            found, hit, any_report, result = coverage_report.render_coverage_entry(
                root, "parent", [("only child", "only/lcov.info")], coverage_summary
            )

        self.assertEqual((found, hit, any_report), (4, 3, True))
        self.assertNotIn("<h3>", result)
        self.assertIn("<code>src/only.py</code>", result)

    def test_render_coverage_entry_sorts_equal_percentages_by_source_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = root / "lcov.info"
            report.write_text(
                "SF:pages/z-last.py\nLF:4\nLH:2\nend_of_record\n"
                "SF:pages/a-first.py\nLF:2\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )
            coverage_summary = coverage_report.load_coverage_summary()

            found, hit, any_report, result = coverage_report.render_coverage_entry(
                root, "Pages", "lcov.info", coverage_summary
            )

        self.assertEqual((found, hit, any_report), (6, 3, True))
        self.assertLess(result.index("pages/a-first.py"), result.index("pages/z-last.py"))

    def test_render_coverage_entry_escapes_nested_group_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            coverage_summary = coverage_report.load_coverage_summary()

            _found, _hit, _any_report, result = coverage_report.render_coverage_entry(
                Path(directory),
                "parent",
                [("<available>", "missing-one.info"), ("safe & sound", "missing-two.info")],
                coverage_summary,
            )

        self.assertIn("<h3>&lt;available&gt; — n/a (0/0)</h3>", result)
        self.assertIn("<h3>safe &amp; sound — n/a (0/0)</h3>", result)
        self.assertNotIn("<available>", result)


class CoveragePageTest(unittest.TestCase):
    def test_build_coverage_page_renders_a_placeholder_without_a_coverage_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<title><!--COVERAGE_VERSION--></title><main><!--COVERAGE_CONTENT--></main>",
                encoding="utf-8",
            )

            with patch.object(coverage_report, "PAGES_DIR", pages_dir):
                coverage_report.build_coverage_page(out_dir, None, "")

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("unreleased", output)
        self.assertIn("Coverage data isn't available for this build.", output)

    def test_build_coverage_page_uses_placeholder_for_a_missing_coverage_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<main><!--COVERAGE_CONTENT--></main>", encoding="utf-8"
            )

            with patch.object(coverage_report, "PAGES_DIR", pages_dir):
                coverage_report.build_coverage_page(out_dir, root / "missing", "v2.0.0")

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("Coverage data isn't available for this build.", output)

    def test_build_coverage_page_renders_report_content_from_a_coverage_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            coverage_dir = root / "coverage-artifacts"
            pages_dir.mkdir()
            out_dir.mkdir()
            coverage_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<title><!--COVERAGE_VERSION--></title><main><!--COVERAGE_CONTENT--></main>",
                encoding="utf-8",
            )
            report_dir = coverage_dir / "rust-coverage-papyrus-parser"
            report_dir.mkdir()
            (report_dir / "lcov.info").write_text("SF:src/lib.rs\nLF:2\nLH:1\nend_of_record\n", encoding="utf-8")

            with patch.object(coverage_report, "PAGES_DIR", pages_dir):
                coverage_report.build_coverage_page(out_dir, coverage_dir, "v1.4.0")

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("v1.4.0", output)
        self.assertIn("papyrus-parser", output)
        self.assertIn("src/lib.rs", output)
        self.assertNotIn("Coverage data isn't available", output)

    def test_build_coverage_page_escapes_the_version_label(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text(
                "<title><!--COVERAGE_VERSION--></title><main><!--COVERAGE_CONTENT--></main>",
                encoding="utf-8",
            )

            with patch.object(coverage_report, "PAGES_DIR", pages_dir):
                coverage_report.build_coverage_page(out_dir, None, 'v1<&"')

            output = (out_dir / "coverage.html").read_text(encoding="utf-8")

        self.assertIn("<title>v1&lt;&amp;&quot;</title>", output)
        self.assertNotIn('v1<&"', output)

    def test_build_coverage_page_rejects_a_template_without_the_content_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            pages_dir.mkdir()
            (pages_dir / "coverage.template.html").write_text("<main>No marker</main>", encoding="utf-8")

            with (
                patch.object(coverage_report, "PAGES_DIR", pages_dir),
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
                coverage_report.build_coverage_page(root / "out", None, "")


if __name__ == "__main__":
    unittest.main()
