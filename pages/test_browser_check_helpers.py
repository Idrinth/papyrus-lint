"""Unit tests for URL classification and the local browser-check server."""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
import urllib.error
import urllib.request
from pathlib import Path

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
