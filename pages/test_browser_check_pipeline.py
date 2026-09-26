"""Focused unit tests for the browser checker's orchestration helpers."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, Mock, patch

from pages import browser_check


class CheckSitePipelineTest(unittest.TestCase):
    def test_check_site_combines_browser_and_local_link_problems(self) -> None:
        server = Mock()
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "z.html").write_text("<html></html>", encoding="utf-8")
            (dist / "a.html").write_text("<html></html>", encoding="utf-8")
            ids = {"a.html": {"top"}}
            links = {"a.html": ["missing.html"]}

            with (
                patch.object(browser_check, "start_server", return_value=(server, "http://local.test")),
                patch.object(
                    browser_check,
                    "inspect_pages",
                    return_value=(ids, links, ["a.html: document error: bad page"]),
                ) as inspect_pages,
            ):
                problems = browser_check.check_site(dist)

        inspect_pages.assert_called_once_with(["a.html", "z.html"], "http://local.test")
        server.shutdown.assert_called_once_with()
        server.server_close.assert_called_once_with()
        self.assertEqual(
            problems,
            [
                "a.html: document error: bad page",
                "a.html: broken link: 'missing.html' (no such file 'missing.html')",
            ],
        )

    def test_inspect_pages_collects_ids_links_and_formatted_issues(self) -> None:
        page = MagicMock()
        page.eval_on_selector_all.side_effect = [["home"], ["guide", "install"]]
        page.content.side_effect = [
            '<a href="guide.html#install">Guide</a>',
            '<a href="index.html#home">Home</a>',
        ]
        browser = MagicMock()
        browser.new_page.return_value = page
        playwright = MagicMock()
        playwright.chromium.launch.return_value = browser
        playwright_context = MagicMock()
        playwright_context.__enter__.return_value = playwright
        first_issues = browser_check.PageIssues(console_errors=["boom"])
        second_issues = browser_check.PageIssues(document_errors=["missing title"])

        with (
            patch.object(browser_check, "sync_playwright", return_value=playwright_context),
            patch.object(
                browser_check,
                "collect_page_issues",
                side_effect=[first_issues, second_issues],
            ) as collect_page_issues,
        ):
            ids, links, problems = browser_check.inspect_pages(
                ["index.html", "guide.html"], "http://local.test"
            )

        self.assertEqual(ids, {"index.html": {"home"}, "guide.html": {"guide", "install"}})
        self.assertEqual(
            links,
            {"index.html": ["guide.html#install"], "guide.html": ["index.html#home"]},
        )
        self.assertEqual(
            problems,
            [
                "index.html: console error: boom",
                "guide.html: document error: missing title",
            ],
        )
        self.assertEqual(collect_page_issues.call_count, 2)
        page.route.assert_called_once()
        browser.close.assert_called_once_with()


class CollectPageIssuesTest(unittest.TestCase):
    def test_collects_runtime_and_document_errors_then_removes_listeners(self) -> None:
        page = MagicMock()
        listeners = {}
        page.on.side_effect = lambda event, callback: listeners.__setitem__(event, callback)
        page.evaluate.return_value = {
            "duplicateIds": ["same"],
            "imagesWithoutAlt": ["photo.png"],
            "unnamedInteractiveElements": ["<button#save>"],
            "hasDocumentLanguage": False,
            "hasDocumentTitle": False,
        }

        def emit_events(*_args, **_kwargs) -> None:
            listeners["console"](Mock(type="error", text="console failed"))
            listeners["console"](Mock(type="warning", text="ignore me"))
            listeners["pageerror"](RuntimeError("script failed"))
            listeners["response"](Mock(url="http://local.test/missing.js", status=404))
            listeners["response"](Mock(url="https://external.test/missing.js", status=500))

        page.goto.side_effect = emit_events

        issues = browser_check.collect_page_issues(page, "http://local.test", "guide.html")

        page.goto.assert_called_once_with("http://local.test/guide.html", wait_until="load", timeout=15000)
        page.evaluate.assert_called_once_with(browser_check.DOCUMENT_CHECKS_SCRIPT)
        self.assertEqual(issues.console_errors, ["console failed"])
        self.assertEqual(issues.page_errors, ["script failed"])
        self.assertEqual(issues.failed_requests, ["http://local.test/missing.js -> 404"])
        self.assertEqual(
            issues.document_errors,
            [
                "duplicate element id 'same'",
                "image 'photo.png' has no alt attribute",
                "interactive element '<button#save>' has no accessible name",
                "document has no language",
                "document has no title",
            ],
        )
        self.assertEqual(page.remove_listener.call_count, 3)


class LocalLinkValidationTest(unittest.TestCase):
    def test_check_local_links_ignores_external_links_and_reports_all_local_failures(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text("home", encoding="utf-8")
            (dist / "guide.html").write_text("guide", encoding="utf-8")

            problems = browser_check.check_local_links(
                dist,
                {"index.html": {"top"}, "guide.html": {"install"}},
                {
                    "index.html": [
                        "https://example.test/missing",
                        "guide.html#install",
                        "guide.html#missing%20section",
                        "missing.html",
                    ]
                },
            )

        self.assertEqual(
            problems,
            [
                "index.html: broken link: 'guide.html#missing%20section' "
                "(no element with id 'missing section' on 'guide.html')",
                "index.html: broken link: 'missing.html' (no such file 'missing.html')",
            ],
        )

    def test_resolve_href_target_handles_same_page_directory_and_encoded_paths(self) -> None:
        self.assertEqual(
            browser_check.resolve_href_target("docs/guide.html", "#intro%20text"),
            ("docs/guide.html", "intro text"),
        )
        self.assertEqual(browser_check.resolve_href_target("index.html", "docs/#start"), ("docs/index.html", "start"))
        self.assertEqual(
            browser_check.resolve_href_target("docs/index.html", "../some%20file.html"),
            ("some file.html", ""),
        )


if __name__ == "__main__":
    unittest.main()
