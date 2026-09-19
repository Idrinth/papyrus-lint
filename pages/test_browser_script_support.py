"""Shared Playwright fixture for the static-script browser tests."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from pages import browser_check


class StaticScriptTestCase(unittest.TestCase):
    def setUpClass(cls) -> None:
        cls.site_directory = tempfile.TemporaryDirectory()
        Path(cls.site_directory.name, "index.html").write_text("<!doctype html>", encoding="utf-8")
        cls.server, cls.base_url = browser_check.start_server(Path(cls.site_directory.name))
        cls.playwright_context = browser_check.sync_playwright()
        cls.playwright = cls.playwright_context.start()
        cls.browser = cls.playwright.chromium.launch()

    def tearDownClass(cls) -> None:
        cls.browser.close()
        cls.playwright.stop()
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

    def is_focused(self, locator) -> bool:
        return locator.evaluate("element => element === document.activeElement")
