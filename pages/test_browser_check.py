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
from unittest.mock import MagicMock, Mock, patch

from pages import browser_check


class IsLocalHrefTest(unittest.TestCase):
    def test_accepts_empty_and_fragment_only_hrefs(self) -> None:
        self.assertTrue(browser_check.is_local_href(""))
        self.assertTrue(browser_check.is_local_href("#section"))

    def test_accepts_relative_paths(self) -> None:
        self.assertTrue(browser_check.is_local_href("other.html"))
        self.assertTrue(browser_check.is_local_href("../assets/style.css"))
        self.assertTrue(browser_check.is_local_href("docs/guide.html#setup"))

    def test_accepts_root_relative_and_query_only_urls(self) -> None:
        self.assertTrue(browser_check.is_local_href("/docs/guide.html"))
        self.assertTrue(browser_check.is_local_href("?print=1"))
        self.assertTrue(browser_check.is_local_href("/?print=1#top"))

    def test_rejects_external_schemes(self) -> None:
        self.assertFalse(browser_check.is_local_href("http://example.com"))
        self.assertFalse(browser_check.is_local_href("https://example.com/page"))
        self.assertFalse(browser_check.is_local_href("mailto:test@example.com"))
        self.assertFalse(browser_check.is_local_href("tel:+15555550100"))

    def test_rejects_scheme_relative_and_non_navigation_urls(self) -> None:
        self.assertFalse(browser_check.is_local_href("//cdn.example.com/style.css"))
        self.assertFalse(browser_check.is_local_href("data:text/plain,hello"))
        self.assertFalse(browser_check.is_local_href("javascript:void(0)"))

    def test_rejects_external_urls_regardless_of_scheme_casing(self) -> None:
        self.assertFalse(browser_check.is_local_href("HTTPS://example.com/page"))
        self.assertFalse(browser_check.is_local_href("HtTp://example.com/page"))


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

    def test_serves_nested_files_and_decodes_url_paths(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            nested = Path(directory) / "release notes"
            nested.mkdir()
            (nested / "index.html").write_text("nested page", encoding="utf-8")

            server, base_url = browser_check.start_server(Path(directory))
            try:
                with urllib.request.urlopen(f"{base_url}/release%20notes/") as response:
                    self.assertEqual(response.status, 200)
                    self.assertEqual(response.read(), b"nested page")
            finally:
                server.shutdown()
                server.server_close()


class StaticScriptsTest(unittest.TestCase):
    """Exercise the site's progressive-enhancement scripts in a real DOM."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.site_directory = tempfile.TemporaryDirectory()
        Path(cls.site_directory.name, "index.html").write_text("<!doctype html>", encoding="utf-8")
        cls.server, cls.base_url = browser_check.start_server(Path(cls.site_directory.name))
        cls.playwright_context = browser_check.sync_playwright()
        cls.playwright = cls.playwright_context.start()
        cls.browser = cls.playwright.chromium.launch()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.browser.close()
        cls.playwright_context.stop()
        cls.server.shutdown()
        cls.server.server_close()
        cls.site_directory.cleanup()

    def new_page(self):
        page = self.browser.new_page()
        self.addCleanup(page.close)
        page.goto(self.base_url)
        return page

    def run_script(self, markup: str, script_name: str):
        page = self.new_page()
        page.set_content(markup)
        page.add_script_tag(path=str(browser_check.PAGES_DIR / script_name))
        return page

    def test_theme_script_restores_and_changes_the_saved_theme(self) -> None:
        page = self.new_page()
        page.set_content(
            '<select id="theme-select"><option>system</option>'
            '<option>light</option><option>dark</option></select>'
        )
        page.evaluate("localStorage.setItem('papyrus-lint:theme', 'dark')")
        page.add_script_tag(path=str(browser_check.PAGES_DIR / "theme.js"))

        self.assertEqual(page.locator("#theme-select").input_value(), "dark")
        page.locator("#theme-select").select_option("light")
        self.assertEqual(page.locator("html").get_attribute("data-theme"), "light")
        self.assertEqual(page.evaluate("localStorage.getItem('papyrus-lint:theme')"), "light")

        page.locator("#theme-select").select_option("system")
        self.assertIsNone(page.locator("html").get_attribute("data-theme"))

    def test_theme_script_ignores_invalid_storage_and_tracks_header_height(self) -> None:
        page = self.new_page()
        page.set_content(
            '<style>.site-header { height: 37px }</style><header class="site-header"></header>'
            '<select id="theme-select"><option>system</option><option>light</option><option>dark</option></select>'
        )
        page.evaluate("localStorage.setItem('papyrus-lint:theme', 'sepia')")
        page.add_script_tag(path=str(browser_check.PAGES_DIR / "theme.js"))

        self.assertEqual(page.locator("#theme-select").input_value(), "system")
        self.assertEqual(
            page.evaluate("getComputedStyle(document.documentElement).getPropertyValue('--site-header-height')"),
            "37px",
        )

    def test_download_script_builds_a_safe_os_specific_picker(self) -> None:
        page = self.run_script(
            """<div class="download-group">
            <a id="download" data-download-toggle data-download-id="gui"
               data-download-label="Choose the GUI build"
               data-options='[{"os":"windows","file":"PapyrusLinter-windows-x64_setup.exe","label":"Windows"},
                              {"os":"linux","file":"PapyrusLinter-linux-amd64.AppImage","label":"Linux"},
                              {"os":"linux","file":"https://evil.test/payload","label":"Unsafe"}]'
               href="https://github.com/idrinth/papyrus-lint/releases/latest">Download GUI</a>
            </div>""",
            "downloads.js",
        )

        self.assertEqual(page.locator("select option").all_text_contents(), ["Windows", "Linux"])
        self.assertEqual(page.locator("select").input_value(), "PapyrusLinter-linux-amd64.AppImage")
        self.assertEqual(page.locator("label").text_content(), "Choose the GUI build")
        self.assertTrue(page.locator(".download-panel").is_hidden())

        page.locator("#download").click()
        self.assertTrue(page.locator("select").is_focused())
        self.assertEqual(page.locator("#download").get_attribute("aria-expanded"), "true")
        self.assertTrue(page.locator(".download-panel__go").get_attribute("href").endswith(".AppImage"))

        page.locator("select").select_option("PapyrusLinter-windows-x64_setup.exe")
        self.assertTrue(page.locator(".download-panel__go").get_attribute("href").endswith("_setup.exe"))

    def test_download_picker_closes_on_outside_click_and_escape(self) -> None:
        page = self.run_script(
            """<div class="download-group"><a id="download" data-download-toggle
               data-options='[{"file":"PapyrusLinterCLI-linux","label":"Linux"}]' href="#fallback">CLI</a></div>
               <button id="outside">Outside</button>""",
            "downloads.js",
        )

        page.locator("#download").click()
        page.locator("#outside").click()
        self.assertTrue(page.locator(".download-panel").is_hidden())
        self.assertEqual(page.locator("#download").get_attribute("aria-expanded"), "false")

        page.locator("#download").click()
        page.keyboard.press("Escape")
        self.assertTrue(page.locator(".download-panel").is_hidden())
        self.assertTrue(page.locator("#download").is_focused())

    def test_download_script_leaves_invalid_configuration_as_a_plain_link(self) -> None:
        page = self.run_script(
            """<a id="missing" data-download-toggle href="#missing">Missing</a>
            <a id="malformed" data-download-toggle data-options="not json" href="#malformed">Malformed</a>
            <a id="object" data-download-toggle data-options='{"file":"PapyrusLinterCLI-linux"}'
               href="#object">Object</a>
            <a id="unknown" data-download-toggle
               data-options='[{"file":"unknown.exe","label":"Unknown"}]'
               href="#unknown">Unknown</a>""",
            "downloads.js",
        )

        self.assertEqual(page.locator(".download-panel").count(), 0)
        for element_id in ("missing", "malformed", "object", "unknown"):
            self.assertIsNone(page.locator(f"#{element_id}").get_attribute("aria-haspopup"))


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

    def test_closes_browser_and_server_when_page_navigation_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text("<html></html>", encoding="utf-8")
            server = MagicMock()
            page = MagicMock()
            page.goto.side_effect = RuntimeError("browser navigation failed")
            browser = MagicMock()
            browser.new_page.return_value = page
            playwright = MagicMock()
            playwright.chromium.launch.return_value = browser
            playwright_context = MagicMock()
            playwright_context.__enter__.return_value = playwright

            def assert_browser_closed_before_playwright_stops(*_args) -> None:
                browser.close.assert_called_once_with()

            playwright_context.__exit__.side_effect = assert_browser_closed_before_playwright_stops

            with (
                patch.object(browser_check, "start_server", return_value=(server, "http://local.test")),
                patch.object(browser_check, "sync_playwright", return_value=playwright_context),
                self.assertRaisesRegex(RuntimeError, "browser navigation failed"),
            ):
                browser_check.check_site(dist)

        browser.close.assert_called_once_with()
        playwright_context.__exit__.assert_called_once()
        server.shutdown.assert_called_once_with()
        server.server_close.assert_called_once_with()

    def test_closes_server_when_browser_launch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text("<html></html>", encoding="utf-8")
            server = MagicMock()
            playwright = MagicMock()
            playwright.chromium.launch.side_effect = RuntimeError("browser launch failed")
            playwright_context = MagicMock()
            playwright_context.__enter__.return_value = playwright

            with (
                patch.object(browser_check, "start_server", return_value=(server, "http://local.test")),
                patch.object(browser_check, "sync_playwright", return_value=playwright_context),
                self.assertRaisesRegex(RuntimeError, "browser launch failed"),
            ):
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

    def test_resolves_fragments_on_directory_index_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            docs = dist / "docs"
            docs.mkdir()
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body>'
                '<a href="docs/#install">Install</a>'
                '<a href="docs/?view=compact#missing">Missing section</a>'
                "</body></html>",
                encoding="utf-8",
            )
            (docs / "index.html").write_text(
                '<html lang="en"><head><title>Docs</title></head><body>'
                '<h1 id="install">Install</h1></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: broken link: 'docs/?view=compact#missing' "
                "(no element with id 'missing' on 'docs/index.html')"
            ],
        )

    def test_accepts_links_to_existing_non_html_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            assets = dist / "assets"
            assets.mkdir()
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Downloads</title></head><body>'
                '<a href="assets/example.psc?download=1">Example script</a>'
                "</body></html>",
                encoding="utf-8",
            )
            (assets / "example.psc").write_text("Scriptname Example", encoding="utf-8")

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])

    def test_checks_html_pages_recursively(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            nested = dist / "guides" / "advanced"
            nested.mkdir(parents=True)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body></body></html>',
                encoding="utf-8",
            )
            (nested / "setup.html").write_text(
                '<html lang="en"><head><title>Setup</title></head><body>'
                '<a href="missing.html">Missing</a></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "guides/advanced/setup.html: broken link: 'missing.html' "
                "(no such file 'guides/advanced/missing.html')"
            ],
        )

    def test_accepts_empty_query_and_fragment_links_to_the_current_page(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title></head><body>'
                '<h1 id="top">Home</h1><a href="">Empty</a><a href="?print=1">Print</a>'
                '<a href="?#top">Top</a></body></html>',
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

    def test_checks_links_added_by_javascript_after_page_load(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Home</title></head><body>
                <script>
                  const link = document.createElement('a');
                  link.href = 'generated-missing.html';
                  link.textContent = 'Generated link';
                  document.body.append(link);
                </script></body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: broken link: 'generated-missing.html' "
                "(no such file 'generated-missing.html')"
            ],
        )

    def test_checks_single_quoted_and_unquoted_href_attributes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Home</title></head><body>
                <a href='single-missing.html'>Single quoted</a>
                <a href=unquoted-missing.html>Unquoted</a>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: broken link: 'single-missing.html' "
                "(no such file 'single-missing.html')",
                "index.html: broken link: 'unquoted-missing.html' "
                "(no such file 'unquoted-missing.html')",
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

    def test_fakes_external_scripts_without_executing_remote_content(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Home</title>'
                '<script src="https://example.test/unavailable.js"></script>'
                '</head><body></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])

    def test_allows_successful_local_stylesheets_scripts_and_images(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Resources</title>
                <link rel="stylesheet" href="styles.css">
                <script src="app.js" defer></script></head><body>
                <img src="pixel.svg" alt="Pixel">
                </body></html>""",
                encoding="utf-8",
            )
            (dist / "styles.css").write_text("body { color: black; }", encoding="utf-8")
            (dist / "app.js").write_text("document.body.dataset.loaded = 'true';", encoding="utf-8")
            (dist / "pixel.svg").write_text(
                '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>',
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

    def test_reports_each_duplicate_id_only_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Duplicate ids</title></head><body>
                <div id="first"></div><div id="first"></div><div id="first"></div>
                <div id="second"></div><div id="second"></div>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: document error: duplicate element id 'first'",
                "index.html: document error: duplicate element id 'second'",
            ],
        )

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

    def test_reports_every_image_without_an_alt_attribute(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                '<html lang="en"><head><title>Images</title></head><body>'
                '<img src="first.svg"><img><img src="third.svg" alt="Third">'
                "</body></html>",
                encoding="utf-8",
            )
            for image_name in ("first.svg", "third.svg"):
                (dist / image_name).write_text(
                    "<svg xmlns='http://www.w3.org/2000/svg'/>",
                    encoding="utf-8",
                )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: document error: image 'first.svg' has no alt attribute",
                "index.html: document error: image '<no src>' has no alt attribute",
            ],
        )

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
