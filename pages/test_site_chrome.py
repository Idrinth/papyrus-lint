"""Tests for the shared page chrome, funding footer, and per-page
finalization."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import site_chrome


class RenderSharedComponentsTest(unittest.TestCase):
    def test_uses_one_source_with_page_relative_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            includes_dir = Path(directory)
            (includes_dir / "header.html").write_text(
                '<header><a href="<!--ROOT_PATH-->index.html">Home</a></header>', encoding="utf-8"
            )
            (includes_dir / "footer.html").write_text(
                "<footer><!--VERSION--></footer>", encoding="utf-8"
            )
            with patch.object(site_chrome, "INCLUDES_DIR", includes_dir):
                result = site_chrome.render_shared_components(
                    "<!--SITE_HEADER--><main>Docs</main><!--SITE_FOOTER-->", "../", "v1.2&3"
                )

        self.assertEqual(
            result,
            '<header><a href="../index.html">Home</a></header>'
            "<main>Docs</main><footer>v1.2&amp;3</footer>",
        )

    def test_rejects_a_partially_shared_shell(self) -> None:
        with self.assertRaisesRegex(SystemExit, "missing shared component marker <!--SITE_FOOTER-->"):
            site_chrome.render_shared_components("<!--SITE_HEADER--><main></main>", "", "")

    def test_rejects_a_footer_without_a_header(self) -> None:
        with self.assertRaisesRegex(SystemExit, "missing shared component marker <!--SITE_HEADER-->"):
            site_chrome.render_shared_components("<main></main><!--SITE_FOOTER-->", "", "")

    def test_supports_fragment_only_templates(self) -> None:
        result = site_chrome.render_shared_components(
            "<title><!--VERSION--></title><a href=\"<!--SITE_URL-->\">Site</a>",
            "../",
            "",
        )

        self.assertEqual(
            result,
            f'<title>unreleased</title><a href="{site_chrome.SITE_URL}">Site</a>',
        )

    def test_replaces_placeholders_inside_includes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            includes_dir = Path(directory)
            (includes_dir / "header.html").write_text(
                '<a href="<!--SITE_URL-->"><!--ROOT_PATH--></a>', encoding="utf-8"
            )
            (includes_dir / "footer.html").write_text(
                "<footer><!--FUNDING_LINKS--><!--VERSION--></footer>", encoding="utf-8"
            )
            with (
                patch.object(site_chrome, "INCLUDES_DIR", includes_dir),
                patch.object(site_chrome, "SITE_URL", "https://example.test/"),
                patch.object(site_chrome, "render_funding_links", return_value="<li>Support</li>"),
            ):
                result = site_chrome.render_shared_components(
                    "<!--SITE_HEADER--><!--SITE_FOOTER-->", "../", 'v1<&"'
                )

        self.assertEqual(
            result,
            '<a href="https://example.test/">../</a>'
            "<footer><li>Support</li>v1&lt;&amp;&quot;</footer>",
        )


class TaggedLinksTest(unittest.TestCase):
    def test_replaces_contact_link_markers_from_shared_yaml_by_tag(self) -> None:
        result = site_chrome.render_shared_components(
            '<div class="badge-row"><!--CONTACT-LINKS--></div>',
            "",
            "",
        )

        self.assertNotIn("<!--CONTACT-LINKS-->", result)
        self.assertIn('href="https://discord.gg/idrinth"', result)
        self.assertIn('href="https://tally.so/r/aQL1dB"', result)
        self.assertNotIn("marketplace.visualstudio.com", result)
        self.assertNotIn("github.com/marketplace/actions", result)

    def test_leaves_templates_without_link_markers_unchanged(self) -> None:
        result = site_chrome.render_shared_components("<main>plain</main>", "", "")

        self.assertEqual(result, "<main>plain</main>")


class FundingLinksTest(unittest.TestCase):
    def test_parse_funding_values_accepts_scalars_lists_quotes_and_empty_values(self) -> None:
        self.assertEqual(site_chrome.parse_funding_values(" sponsor "), ["sponsor"])
        self.assertEqual(
            site_chrome.parse_funding_values("['first sponsor', \"second\", '']"),
            ["first sponsor", "second"],
        )
        self.assertEqual(site_chrome.parse_funding_values("   "), [])

    def test_parse_funding_values_ignores_empty_inline_list_entries(self) -> None:
        self.assertEqual(
            site_chrome.parse_funding_values("[first, , '', \"second account\"]"),
            ["first", "second account"],
        )

    def test_render_funding_links_reads_provider_and_custom_links(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text(
                "github: [first sponsor, second]\ncustom: https://www.paypal.com/donate?id=1&campaign=two\n",
                encoding="utf-8",
            )

            result = site_chrome.render_funding_links(funding_file)

        self.assertIn('href="https://github.com/sponsors/first%20sponsor"', result)
        self.assertIn('href="https://github.com/sponsors/second"', result)
        self.assertIn(">GitHub Sponsors</a>", result)
        self.assertIn('href="https://www.paypal.com/donate?id=1&amp;campaign=two"', result)
        self.assertIn(">PayPal</a>", result)

    def test_render_funding_links_rejects_an_invalid_custom_url(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text("custom: javascript:alert(1)\n", encoding="utf-8")

            with self.assertRaisesRegex(SystemExit, "must be an HTTP\\(S\\) URL"):
                site_chrome.render_funding_links(funding_file)

    def test_render_funding_links_rejects_custom_urls_without_a_web_host(self) -> None:
        invalid_urls = ("https:///missing-host", "mailto:maintainer@example.test")

        for invalid_url in invalid_urls:
            with self.subTest(url=invalid_url), tempfile.TemporaryDirectory() as directory:
                funding_file = Path(directory) / "FUNDING.yml"
                funding_file.write_text(f"custom: {invalid_url}\n", encoding="utf-8")

                with self.assertRaisesRegex(
                    SystemExit,
                    f"custom funding link must be an HTTP\\(S\\) URL: {invalid_url}",
                ):
                    site_chrome.render_funding_links(funding_file)

    def test_render_funding_links_skips_comments_malformed_lines_and_unknown_providers(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text(
                "# Maintainer funding\nmalformed line\nunknown: account\ngithub: valid-user\n",
                encoding="utf-8",
            )

            result = site_chrome.render_funding_links(funding_file)

        self.assertEqual(result.count("<li>"), 1)
        self.assertIn("https://github.com/sponsors/valid-user", result)
        self.assertNotIn("unknown", result)

    def test_render_funding_links_supports_each_named_provider(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text(
                "\n".join(f"{provider}: account/name" for provider in site_chrome.FUNDING_PROVIDERS),
                encoding="utf-8",
            )

            result = site_chrome.render_funding_links(funding_file)

        self.assertEqual(result.count("<li>"), len(site_chrome.FUNDING_PROVIDERS))
        for label, url_template in site_chrome.FUNDING_PROVIDERS.values():
            self.assertIn(f">{label}</a>", result)
            self.assertIn(url_template.format("account%2Fname"), result)

    def test_render_funding_links_labels_non_paypal_custom_urls_generically(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text("custom: https://example.test/support\n", encoding="utf-8")

            result = site_chrome.render_funding_links(funding_file)

        self.assertIn(">Support this project</a>", result)
        self.assertIn('href="https://example.test/support"', result)

    def test_render_funding_links_encodes_provider_handles_as_path_segments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            funding_file = Path(directory) / "FUNDING.yml"
            funding_file.write_text("github: user/name\n", encoding="utf-8")

            result = site_chrome.render_funding_links(funding_file)

        self.assertIn("https://github.com/sponsors/user%2Fname", result)
        self.assertNotIn("sponsors/user/name", result)


class FinalizePageTest(unittest.TestCase):
    def test_wraps_modern_format_images_and_minifies(self) -> None:
        with patch.object(site_chrome, "wrap_images_with_modern_sources", return_value="<main>wrapped</main>"):
            result = site_chrome.finalize_page("<main>   original   </main>")

        self.assertEqual(result, "<main>wrapped</main>")


if __name__ == "__main__":
    unittest.main()
