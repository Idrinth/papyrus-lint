"""Browser tests for the shared theme script."""

from __future__ import annotations

from pages import browser_check
from pages.test_browser_script_support import StaticScriptTestCase


class ThemeScriptTest(StaticScriptTestCase):
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

    def test_theme_script_tolerates_pages_without_optional_header_controls(self) -> None:
        page = self.run_script("<main>Standalone content</main>", "theme.js")

        self.assertEqual(page.locator("main").text_content(), "Standalone content")
        self.assertIsNone(page.locator("html").get_attribute("data-theme"))
        self.assertEqual(
            page.evaluate(
                "getComputedStyle(document.documentElement)"
                ".getPropertyValue('--site-header-height')"
            ),
            "",
        )

    def test_theme_script_tolerates_unavailable_local_storage(self) -> None:
        page = self.new_page()
        page.set_content(
            '<select id="theme-select"><option>system</option>'
            '<option>light</option><option>dark</option></select>'
        )
        page.evaluate(
            """() => {
                Storage.prototype.getItem = () => { throw new Error('storage blocked'); };
                Storage.prototype.setItem = () => { throw new Error('storage blocked'); };
            }"""
        )
        page.add_script_tag(path=str(browser_check.PAGES_DIR / "theme.js"))

        self.assertEqual(page.locator("#theme-select").input_value(), "system")
        page.locator("#theme-select").select_option("dark")
        self.assertEqual(page.locator("html").get_attribute("data-theme"), "dark")

    def test_theme_script_falls_back_to_window_resize_without_resize_observer(self) -> None:
        page = self.new_page()
        page.set_content('<style>.site-header { height: 24px }</style><header class="site-header"></header>')
        page.evaluate("delete window.ResizeObserver")
        page.add_script_tag(path=str(browser_check.PAGES_DIR / "theme.js"))

        page.locator(".site-header").evaluate("element => element.style.height = '51px'")
        page.evaluate("window.dispatchEvent(new Event('resize'))")
        self.assertEqual(
            page.evaluate(
                "getComputedStyle(document.documentElement)"
                ".getPropertyValue('--site-header-height')"
            ),
            "51px",
        )
