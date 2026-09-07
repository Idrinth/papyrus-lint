"""Tests for the Playwright-based built-site checker.

`pages/test_build.py` only exercises `build.py`'s own Markdown/table
conversion logic against small in-memory fixtures; `browser_check.py`'s own
logic (which local hrefs to check, how a link target and its fragment are
resolved back to a file on disk, and what counts as a console/page/resource
issue) previously had no unit coverage of its own at all, only the indirect,
whole-site coverage of the `pages-browser` CI job actually running it against
the real built site.
"""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
import urllib.error
import urllib.request
from pathlib import Path
from unittest.mock import patch

from pages import browser_check


class IsLocalHrefTest(unittest.TestCase):
    def test_accepts_empty_and_fragment_only_hrefs(self) -> None:
        self.assertTrue(browser_check.is_local_href(""))
        self.assertTrue(browser_check.is_local_href("#section"))

    def test_accepts_relative_paths(self) -> None:
        self.assertTrue(browser_check.is_local_href("other.html"))
        self.assertTrue(browser_check.is_local_href("../assets/style.css"))
        self.assertTrue(browser_check.is_local_href("docs/guide.html#setup"))

    def test_rejects_external_schemes(self) -> None:
        self.assertFalse(browser_check.is_local_href("http://example.com"))
        self.assertFalse(browser_check.is_local_href("https://example.com/page"))
        self.assertFalse(browser_check.is_local_href("mailto:test@example.com"))
        self.assertFalse(browser_check.is_local_href("tel:+15555550100"))


class StartServerTest(unittest.TestCase):
    def test_serves_directory_contents_quietly(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            (Path(directory) / "index.html").write_text("hello world", encoding="utf-8")

            server, base_url = browser_check.start_server(Path(directory))
            try:
                with urllib.request.urlopen(f"{base_url}/index.html") as response:
                    self.assertEqual(response.read(), b"hello world")

                stderr = io.StringIO()
                with contextlib.redirect_stderr(stderr):
                    try:
                        urllib.request.urlopen(f"{base_url}/missing.html")
                        self.fail("expected a 404 for a missing file")
                    except urllib.error.HTTPError as error:
                        self.assertEqual(error.code, 404)
                        error.close()
                self.assertEqual(stderr.getvalue(), "")
            finally:
                server.shutdown()
                server.server_close()


class CheckSiteTest(unittest.TestCase):
    def test_reports_no_html_files_without_starting_a_browser(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(browser_check, "sync_playwright") as sync_playwright,
        ):
            problems = browser_check.check_site(Path(directory))

        self.assertEqual(len(problems), 1)
        self.assertIn("no .html files found under", problems[0])
        sync_playwright.assert_not_called()

    def test_detects_broken_links_missing_fragments_and_ignores_valid_ones(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html><body>
                <h1 id="top">Home</h1>
                <a href="other.html">Other page</a>
                <a href="other.html#section">Valid anchor</a>
                <a href="other.html?x=1#section">Anchor past a query string</a>
                <a href="other.html#missing">Missing anchor</a>
                <a href="missing.html">Missing page</a>
                <a href="#top">Same-page anchor</a>
                <a href="https://example.test/elsewhere#nonexistent">External link</a>
                </body></html>""",
                encoding="utf-8",
            )
            (dist / "other.html").write_text(
                """<html><body>
                <h2 id="section">Section</h2>
                <a href="index.html">Back</a>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            sorted(problems),
            sorted(
                [
                    "index.html: broken link: 'other.html#missing' "
                    "(no element with id 'missing' on 'other.html')",
                    "index.html: broken link: 'missing.html' (no such file 'missing.html')",
                ]
            ),
        )

    def test_detects_console_errors_page_errors_and_failed_resources(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "issues.html").write_text(
                """<html><body>
                <script>console.error('boom from console');</script>
                <img src="missing.png" alt="" />
                <script>throw new Error('boom from page');</script>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertTrue(
            any(p == "issues.html: console error: boom from console" for p in problems), problems
        )
        self.assertTrue(any("issues.html: page error:" in p and "boom from page" in p for p in problems), problems)
        self.assertTrue(
            any(p.startswith("issues.html: failed resource:") and p.endswith("-> 404") for p in problems),
            problems,
        )

    def test_fakes_external_requests_instead_of_fetching_them(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html><body><img src="https://example.test/definitely-not-real.png" alt="" />'
                "</body></html>",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])


class MainTest(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
