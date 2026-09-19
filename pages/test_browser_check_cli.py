"""Tests for the browser-check command-line entry point."""

from __future__ import annotations

import contextlib
import io
import runpy
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import browser_check


class MainTest(unittest.TestCase):
    def test_script_entry_point_exits_with_the_main_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "missing"
            with (
                patch("sys.argv", [str(browser_check.__file__), "--dist", str(missing)]),
                contextlib.redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit) as raised,
            ):
                runpy.run_path(str(browser_check.__file__), run_name="__main__")

        self.assertEqual(raised.exception.code, 2)

    def test_reports_an_error_when_the_dist_directory_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            missing = Path(directory) / "does-not-exist"
            stderr = io.StringIO()

            with (
                patch("sys.argv", ["browser_check.py", "--dist", str(missing)]),
                contextlib.redirect_stderr(stderr),
            ):
                exit_code = browser_check.main()

        self.assertEqual(exit_code, 2)
        self.assertIn("does not exist", stderr.getvalue())

    def test_reports_found_issues_and_a_failing_exit_code(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            stdout = io.StringIO()

            with (
                patch("sys.argv", ["browser_check.py", "--dist", directory]),
                patch.object(browser_check, "check_site", return_value=["index.html: broken link: 'x'"]),
                contextlib.redirect_stdout(stdout),
            ):
                exit_code = browser_check.main()

        self.assertEqual(exit_code, 1)
        self.assertIn("Found 1 issue(s):", stdout.getvalue())
        self.assertIn("index.html: broken link: 'x'", stdout.getvalue())

    def test_reports_success_when_no_issues_are_found(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            stdout = io.StringIO()

            with (
                patch("sys.argv", ["browser_check.py", "--dist", directory]),
                patch.object(browser_check, "check_site", return_value=[]),
                contextlib.redirect_stdout(stdout),
            ):
                exit_code = browser_check.main()

        self.assertEqual(exit_code, 0)
        self.assertEqual(stdout.getvalue(), "No issues found.\n")

    def test_defaults_to_a_dist_directory_under_pages_dir(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            pages_dir = Path(directory)
            (pages_dir / "dist").mkdir()
            stdout = io.StringIO()

            with (
                patch("sys.argv", ["browser_check.py"]),
                patch.object(browser_check, "PAGES_DIR", pages_dir),
                patch.object(browser_check, "check_site", return_value=[]) as check_site,
                contextlib.redirect_stdout(stdout),
            ):
                exit_code = browser_check.main()

        self.assertEqual(exit_code, 0)
        check_site.assert_called_once_with(pages_dir / "dist")
