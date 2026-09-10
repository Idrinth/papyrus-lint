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
from unittest.mock import Mock, patch

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

    def test_rejects_scheme_relative_and_non_navigation_urls(self) -> None:
        self.assertFalse(browser_check.is_local_href("//cdn.example.com/style.css"))
        self.assertFalse(browser_check.is_local_href("data:text/plain,hello"))
        self.assertFalse(browser_check.is_local_href("javascript:void(0)"))


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

    def test_stops_the_server_when_playwright_fails_to_start(self) -> None:
        server = Mock()
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(browser_check, "start_server", return_value=(server, "http://127.0.0.1:1234")),
            patch.object(browser_check, "sync_playwright", side_effect=RuntimeError("browser unavailable")),
        ):
            dist = Path(directory)
            (dist / "index.html").write_text("<html></html>", encoding="utf-8")

            with self.assertRaisesRegex(RuntimeError, "browser unavailable"):
                browser_check.check_site(dist)

        server.shutdown.assert_called_once_with()
        server.server_close.assert_called_once_with()

    def test_detects_broken_links_missing_fragments_and_ignores_valid_ones(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Home</title></head><body>
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
                """<html lang="en"><head><title>Other</title></head><body>
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

    def test_resolves_root_relative_parent_and_query_only_links_from_nested_pages(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            docs = dist / "docs"
            docs.mkdir()
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head>'
                '<body><main id="home">Home</main></body></html>',
                encoding="utf-8",
            )
            (docs / "guide.html").write_text(
                """<html lang="en"><head><title>Guide</title></head><body>
                <h1 id="guide">Guide</h1>
                <a href="../index.html#home">Parent-relative home</a>
                <a href="/index.html#home">Root-relative home</a>
                <a href="?mode=print#guide">Query on this page</a>
                <a href="./missing.html">Missing sibling</a>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            ["docs/guide.html: broken link: './missing.html' (no such file 'docs/missing.html')"],
        )

    def test_accepts_directory_index_links_and_the_site_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            docs = dist / "docs"
            docs.mkdir()
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body>'
                '<a href="docs/">Docs</a></body></html>',
                encoding="utf-8",
            )
            (docs / "index.html").write_text(
                '<html lang="en"><head><title>Docs</title></head><body>'
                '<a href="/">Home</a></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])

    def test_decodes_percent_encoded_paths_and_fragments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body>'
                '<a href="release%20notes.html#command%20line">Release notes</a>'
                '<a href="#same%20page">Same page</a>'
                '<h2 id="same page">Same-page heading</h2>'
                '</body></html>',
                encoding="utf-8",
            )
            (dist / "release notes.html").write_text(
                '<html lang="en"><head><title>Release notes</title></head><body>'
                '<h1 id="command line">Command line</h1></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])

    def test_reports_a_decoded_missing_fragment_name(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body>'
                '<a href="#missing%20section">Missing</a></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: broken link: '#missing%20section' "
                "(no element with id 'missing section' on 'index.html')"
            ],
        )

    def test_detects_a_missing_fragment_on_the_same_page(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head>'
                '<body><a href="#missing">Missing section</a></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            ["index.html: broken link: '#missing' (no element with id 'missing' on 'index.html')"],
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

    def test_keeps_browser_events_scoped_to_the_page_that_emitted_them(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "first.html").write_text(
                """<html lang="en"><head><title>First</title></head><body>
                <script>console.error('first page only');</script>
                </body></html>""",
                encoding="utf-8",
            )
            (dist / "second.html").write_text(
                """<html lang="en"><head><title>Second</title></head><body>
                <script>console.error('second page only');</script>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "first.html: console error: first page only",
                "second.html: console error: second page only",
            ],
        )

    def test_fakes_external_requests_instead_of_fetching_them(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body>'
                '<img src="https://example.test/definitely-not-real.png" alt="" />'
                "</body></html>",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])

    def test_detects_duplicate_ids(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Duplicate ids</title></head><body>
                <section id="repeated"></section><div id="repeated"></div>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, ["index.html: document error: duplicate element id 'repeated'"])

    def test_detects_images_without_alt_text(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html><body><img src="photo.png"></body></html>',
                encoding="utf-8",
            )
            (dist / "photo.png").write_bytes(b"not a real image")

            problems = browser_check.check_site(dist)

        self.assertIn("index.html: document error: image 'photo.png' has no alt attribute", problems)

    def test_describes_an_image_without_src_or_alt(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Image</title></head><body><img></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, ["index.html: document error: image '<no src>' has no alt attribute"])

    def test_accepts_empty_alt_text_for_decorative_images(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Accessible page</title></head><body>
                <img src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==" alt="">
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])

    def test_detects_missing_document_language_and_title(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                "<html><head><title>   </title></head><body></body></html>",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: document error: document has no language",
                "index.html: document error: document has no title",
            ],
        )

    def test_accepts_nonempty_document_language_and_title(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Page title</title></head><body></body></html>',
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
