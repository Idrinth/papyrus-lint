"""End-to-end browser-check tests for browser events and page resources."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from pages import browser_check


class CheckSiteRuntimeTest(unittest.TestCase):

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
