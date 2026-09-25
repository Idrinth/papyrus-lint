"""Repository-level checks for the site's templates."""

import unittest

from pages import build as page_builder


class PageTemplatesTest(unittest.TestCase):
    def test_page_templates_have_complete_shared_chrome_and_required_markers(self) -> None:
        required_markers = {
            "index.template.html": {"<!--CLI_EXAMPLES-->", "<!--DOCS_LIST-->", "<!--CONTACT-LINKS-->"},
            "videos.template.html": {"<!--VIDEOS_LIST-->"},
            "action.template.html": {"<!--ACTION_TITLE-->", "<!--ACTION_DESCRIPTION-->", "<!--ACTION_CONTENT-->"},
            "coverage.template.html": {"<!--COVERAGE_VERSION-->", "<!--COVERAGE_CONTENT-->"},
            "imprint.template.html": set(),
            "docs.template.html": {"<!--DOC_TITLE-->", "<!--DOC_DESCRIPTION-->", "<!--DOC_URL-->", "<!--DOC_CONTENT-->"},
        }

        for filename, markers in required_markers.items():
            template = (page_builder.PAGES_DIR / filename).read_text(encoding="utf-8")
            with self.subTest(template=filename):
                self.assertIn("<!--SITE_HEADER-->", template)
                self.assertIn("<!--SITE_FOOTER-->", template)
                for marker in markers:
                    self.assertIn(marker, template)

    def test_homepage_thanks_every_named_contributor(self) -> None:
        template = (page_builder.PAGES_DIR / "index.template.html").read_text(encoding="utf-8")

        self.assertIn('<section id="thanks">', template)
        for contributor in ("WraithFallen", "Scrivener07", "s3ngine", "wall416", "DavidJCobb", "Vict"):
            with self.subTest(contributor=contributor):
                self.assertIn(contributor, template)
        self.assertIn('href="https://x.com/VictMangle"', template)
