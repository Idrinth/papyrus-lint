"""End-to-end browser-check tests for local links and fragments."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from pages import browser_check


class CheckSiteLinksTest(unittest.TestCase):
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
