"""Browser tests for rule filtering and responsive rule details."""

from __future__ import annotations

from pages import browser_check
from pages.css import inline_css_imports
from pages.test_browser_script_support import StaticScriptTestCase


class RulesScriptTest(StaticScriptTestCase):
    def load_rules_table(self, width: int, height: int = 800):
        """Render a two-row rules table with the real site stylesheet."""
        stylesheet = inline_css_imports(
            (browser_check.PAGES_DIR / "styles.css").read_text(encoding="utf-8"),
            browser_check.PAGES_DIR / "styles.css",
        )
        page = self.new_page()
        page.set_viewport_size({"width": width, "height": height})
        page.set_content(
            "<!doctype html><html><head><style>"
            + stylesheet
            + '</style></head><body><main><section class="doc-content rules-content">'
            '<div class="rules-filter-bar">'
            '<input type="search" id="rules-search" />'
            '<label class="filter-chip"><input type="checkbox" class="rules-severity-filter" '
            'value="warning" checked /> Warning</label>'
            '<label class="filter-chip"><input type="checkbox" class="rules-tag-filter" '
            'value="style" checked /> Style</label>'
            '<label class="filter-chip"><input type="checkbox" class="rules-tag-filter" '
            'value="maintainability" checked /> Maintainability</label>'
            '<label class="filter-chip"><input type="checkbox" class="rules-tag-filter" '
            'value="performance" checked /> Performance</label>'
            "</div>"
            '<p class="rules-count" id="rules-count"></p>'
            '<div class="lint-table-wrap"><table class="lint-table lint-rules-table" id="rules-table">'
            "<thead><tr><th>Rule</th><th>Severity</th><th>Tags</th>"
            "<th>Description</th><th>Auto-fix</th></tr></thead><tbody>"
            '<tr id="rule-one" data-severity="warning" data-tags="style" data-fixable="true" '
            'data-search="one first rule">'
            '<td><a href="#rule-one">First rule</a><br /><code>one</code></td>'
            '<td><span class="severity-badge severity-badge--warning">warning</span></td>'
            '<td><span class="tag-badge">maintainability</span>'
            '<span class="tag-badge">performance</span></td>'
            "<td>Short description."
            "<details><summary>Full behavior</summary>"
            "<p>A long definition with "
            "<code>VeryLongUnbrokenIdentifierNameThatWouldOverflowTheCell</code>.</p>"
            "</details></td>"
            '<td class="fix-yes">✓</td></tr>'
            '<tr id="rule-two" data-severity="warning" data-tags="style" data-fixable="false" '
            'data-search="two second rule">'
            '<td><a href="#rule-two">Second rule</a><br /><code>two</code></td>'
            '<td><span class="severity-badge severity-badge--warning">warning</span></td>'
            '<td><span class="tag-badge">style</span></td>'
            "<td>Another description."
            "<details><summary>Full behavior</summary>"
            "<p>The other rule's full behavior.</p>"
            "</details></td>"
            "<td></td></tr>"
            "</tbody></table></div></section></main></body></html>"
        )
        page.add_script_tag(path=str(browser_check.PAGES_DIR / "rules.js"))
        return page

    def test_rules_details_toggle_on_a_narrow_viewport(self) -> None:
        page = self.load_rules_table(390, 844)
        summary = page.locator("#rule-one summary")
        details = page.locator("#rule-one details")

        width = summary.evaluate("element => element.getBoundingClientRect().width")
        self.assertGreater(width, 40)
        self.assertFalse(details.evaluate("element => element.open"))

        summary.scroll_into_view_if_needed()
        summary.click()
        self.assertTrue(details.evaluate("element => element.open"))
        summary.click()
        self.assertFalse(details.evaluate("element => element.open"))

    def test_rules_details_stay_clickable_after_repeated_toggles(self) -> None:
        page = self.load_rules_table(1280)
        first = page.locator("#rule-one summary")
        second = page.locator("#rule-two summary")
        first_details = page.locator("#rule-one details")
        second_details = page.locator("#rule-two details")

        for _ in range(7):
            first.scroll_into_view_if_needed()
            first.click()
            second.scroll_into_view_if_needed()
            second.click()

        self.assertTrue(first_details.evaluate("element => element.open"))
        self.assertTrue(second_details.evaluate("element => element.open"))
        first.click()
        second.click()
        self.assertFalse(first_details.evaluate("element => element.open"))
        self.assertFalse(second_details.evaluate("element => element.open"))

    def test_rules_filter_still_hides_rows_in_the_stacked_layout(self) -> None:
        page = self.load_rules_table(390, 844)
        page.fill("#rules-search", "second")
        self.assertTrue(page.locator("#rule-one").is_hidden())
        self.assertFalse(page.locator("#rule-two").is_hidden())
        page.locator("#rule-two summary").click()
        self.assertTrue(page.locator("#rule-two details").evaluate("element => element.open"))
