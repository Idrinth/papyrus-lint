"""Browser tests for the progressive-enhancement download picker."""

from __future__ import annotations

from pages.test_browser_script_support import StaticScriptTestCase


class DownloadsScriptTest(StaticScriptTestCase):
    def test_download_script_builds_a_safe_os_specific_picker(self) -> None:
        page = self.run_script(
            """<div class="download-group">
            <a id="download" data-download-toggle data-download-id="gui"
               data-download-label="Choose the GUI build"
               data-options='[{"os":"windows","file":"PapyrusLinter-windows-x64_setup.exe","label":"Windows"},
                              {"os":"linux","file":"PapyrusLinter-linux-amd64.AppImage","label":"Linux"},
                              {"os":"linux","file":"https://evil.test/payload","label":"Unsafe"}]'
               href="https://github.com/idrinth/papyrus-lint/releases/latest">Download GUI</a>
            </div>""",
            "downloads.js",
        )

        self.assertEqual(page.locator("select option").all_text_contents(), ["Windows", "Linux"])
        self.assertEqual(page.locator("select").input_value(), "PapyrusLinter-linux-amd64.AppImage")
        self.assertEqual(page.locator("label").text_content(), "Choose the GUI build")
        self.assertTrue(page.locator(".download-panel").is_hidden())

        page.locator("#download").click()
        self.assertTrue(self.is_focused(page.locator("select")))
        self.assertEqual(page.locator("#download").get_attribute("aria-expanded"), "true")
        self.assertTrue(page.locator(".download-panel__go").get_attribute("href").endswith(".AppImage"))

        page.locator("select").select_option("PapyrusLinter-windows-x64_setup.exe")
        self.assertTrue(page.locator(".download-panel__go").get_attribute("href").endswith("_setup.exe"))

    def test_download_picker_closes_on_outside_click_and_escape(self) -> None:
        page = self.run_script(
            """<div class="download-group"><a id="download" data-download-toggle
               data-options='[{"file":"PapyrusLinterCLI-linux","label":"Linux"}]' href="#fallback">CLI</a></div>
               <button id="outside">Outside</button>""",
            "downloads.js",
        )

        page.locator("#download").click()
        page.locator("#outside").click()
        self.assertTrue(page.locator(".download-panel").is_hidden())
        self.assertEqual(page.locator("#download").get_attribute("aria-expanded"), "false")

        page.locator("#download").click()
        page.keyboard.press("Escape")
        self.assertTrue(page.locator(".download-panel").is_hidden())
        self.assertTrue(self.is_focused(page.locator("#download")))

    def test_download_picker_offers_vscode_marketplace_and_vsix(self) -> None:
        page = self.run_script(
            """<div class="download-group"><a id="download" data-download-toggle
               data-download-id="vscode"
               data-options='[{"file":"vscode-marketplace","label":"VS Code Marketplace"},
                              {"file":"papyrus-lint-vscode.vsix","label":"Download VSIX package"}]'
               href="https://marketplace.visualstudio.com/items?itemName=Idrinth.papyrus-lint-vscode">
               VS Code Extension</a></div>""",
            "downloads.js",
        )

        self.assertEqual(
            page.locator("select option").all_text_contents(),
            ["VS Code Marketplace", "Download VSIX package"],
        )
        self.assertIn(
            "marketplace.visualstudio.com",
            page.locator(".download-panel__go").get_attribute("href"),
        )
        page.locator("#download").click()
        page.locator("select").select_option("papyrus-lint-vscode.vsix")
        self.assertTrue(
            page.locator(".download-panel__go")
            .get_attribute("href")
            .endswith("papyrus-lint-vscode.vsix")
        )

    def test_download_picker_toggle_closes_its_open_panel(self) -> None:
        page = self.run_script(
            """<div class="download-group"><a id="download" data-download-toggle
               data-options='[{"file":"PapyrusLinterCLI-linux","label":"Linux"}]'
               href="#fallback">CLI</a></div>""",
            "downloads.js",
        )

        page.locator("#download").click()
        self.assertFalse(page.locator(".download-panel").is_hidden())

        page.locator("#download").click()
        self.assertTrue(page.locator(".download-panel").is_hidden())
        self.assertEqual(page.locator("#download").get_attribute("aria-expanded"), "false")

    def test_download_script_leaves_invalid_configuration_as_a_plain_link(self) -> None:
        page = self.run_script(
            """<a id="missing" data-download-toggle href="#missing">Missing</a>
            <a id="malformed" data-download-toggle data-options="not json" href="#malformed">Malformed</a>
            <a id="object" data-download-toggle data-options='{"file":"PapyrusLinterCLI-linux"}'
               href="#object">Object</a>
            <a id="unknown" data-download-toggle
               data-options='[{"file":"unknown.exe","label":"Unknown"}]'
               href="#unknown">Unknown</a>""",
            "downloads.js",
        )

        self.assertEqual(page.locator(".download-panel").count(), 0)
        for element_id in ("missing", "malformed", "object", "unknown"):
            self.assertIsNone(page.locator(f"#{element_id}").get_attribute("aria-haspopup"))

    def test_download_script_uses_default_accessible_label_and_id(self) -> None:
        page = self.run_script(
            """<div><a id="download" data-download-toggle
               data-options='[{"file":"PapyrusLinterCLI-linux","label":"Linux CLI"}]'
               href="#fallback">CLI</a></div>""",
            "downloads.js",
        )

        self.assertEqual(page.locator("label").get_attribute("for"), "download-download-select")
        self.assertEqual(page.locator("label").text_content(), "Choose a download")
        self.assertEqual(page.locator("select").get_attribute("id"), "download-download-select")
        self.assertEqual(page.locator("select option").text_content(), "Linux CLI")

    def test_download_script_enhances_every_valid_toggle_independently(self) -> None:
        page = self.run_script(
            """<div class="download-group"><a id="gui" data-download-toggle data-download-id="gui"
               data-options='[{"file":"PapyrusLinter-windows-x64.msi","label":"Windows"}]'
               href="#gui-fallback">GUI</a></div>
            <div class="download-group"><a id="cli" data-download-toggle data-download-id="cli"
               data-options='[{"file":"PapyrusLinterCLI-macos","label":"macOS"}]'
               href="#cli-fallback">CLI</a></div>""",
            "downloads.js",
        )

        self.assertEqual(page.locator(".download-panel").count(), 2)
        page.locator("#gui").click()
        self.assertFalse(page.locator("#gui-download-select").is_hidden())
        self.assertTrue(page.locator("#cli-download-select").is_hidden())

        page.locator("#cli").click()
        self.assertTrue(page.locator("#gui-download-select").is_hidden())
        self.assertFalse(page.locator("#cli-download-select").is_hidden())
