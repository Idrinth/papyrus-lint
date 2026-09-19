"""Tests for browser-check startup, cleanup, and empty input handling."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

from pages import browser_check


class CheckSiteLifecycleTest(unittest.TestCase):
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
