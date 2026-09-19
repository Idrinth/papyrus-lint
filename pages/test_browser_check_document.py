"""End-to-end browser-check tests for document structure and accessibility."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from pages import browser_check


class CheckSiteDocumentTest(unittest.TestCase):

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

    def test_detects_interactive_elements_without_accessible_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Controls</title></head><body>
                <a href="target.html"></a><button id="submit"></button>
                <input type="text" name="query"><select id="choice"><option></option></select>
                <textarea></textarea><input type="hidden">
                </body></html>""",
                encoding="utf-8",
            )
            (dist / "target.html").write_text(
                '<html lang="en"><head><title>Target</title></head><body></body></html>',
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(
            problems,
            [
                "index.html: document error: interactive element '<a> (target.html)' has no accessible name",
                "index.html: document error: interactive element '<button#submit>' has no accessible name",
                "index.html: document error: interactive element '<input> (query)' has no accessible name",
                "index.html: document error: interactive element '<select#choice>' has no accessible name",
                "index.html: document error: interactive element '<textarea>' has no accessible name",
            ],
        )

    def test_accepts_the_supported_sources_of_accessible_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            dist = Path(directory)
            (dist / "index.html").write_text(
                """<html lang="en"><head><title>Controls</title></head><body>
                <a href="#"><img alt="Home"></a>
                <button aria-label="Close"></button>
                <span id="search-name">Search</span><input aria-labelledby="search-name">
                <label>Theme <select><option>Dark</option></select></label>
                <label for="notes">Notes</label><textarea id="notes"></textarea>
                <input type="submit" value="Save"><input type="image" alt="Upload">
                <button title="More options"></button>
                </body></html>""",
                encoding="utf-8",
            )

            problems = browser_check.check_site(dist)

        self.assertEqual(problems, [])
