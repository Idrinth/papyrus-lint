#!/usr/bin/env python3
"""Unit tests for shared/links.yaml loading, tag filtering, and rendering."""

import tempfile
import unittest
from pathlib import Path

from ci_lib.links import (
    LINKS_FILE,
    Link,
    filter_by_tag,
    load_links,
    parse_links,
    render_bbcode_list,
    render_html_anchors,
    render_html_list_items,
    render_markdown_list_items,
    render_plain_text,
    replace_angle_link_markers,
    replace_html_link_markers,
)

SAMPLE = """\
Discord:
  url: https://discord.example/invite
  type:
  - contact
Nexus Mods:
  url: https://nexus.example/mod
  type:
  - contact
  - download
Docs:
  url: https://docs.example/
  type:
  - documentation
"""


def sample_links() -> list[Link]:
    return parse_links(SAMPLE)


class ParseLinksTests(unittest.TestCase):
    def test_parse_preserves_order_labels_urls_and_tags(self) -> None:
        links = sample_links()

        self.assertEqual(["Discord", "Nexus Mods", "Docs"], [link.label for link in links])
        self.assertEqual(
            [
                "https://discord.example/invite",
                "https://nexus.example/mod",
                "https://docs.example/",
            ],
            [link.url for link in links],
        )
        self.assertEqual(frozenset({"contact"}), links[0].tags)
        self.assertEqual(frozenset({"contact", "download"}), links[1].tags)
        self.assertEqual(frozenset({"documentation"}), links[2].tags)

    def test_parse_skips_comments_and_blank_lines(self) -> None:
        links = parse_links("# heading\n\nAlpha:\n  url: https://alpha.example\n  type:\n  - contact\n")

        self.assertEqual(["Alpha"], [link.label for link in links])

    def test_load_links_reads_the_repository_file(self) -> None:
        links = load_links()

        self.assertTrue(LINKS_FILE.is_file())
        self.assertTrue(links)
        self.assertTrue(all(link.label and link.url and link.tags for link in links))

    def test_rejects_duplicate_labels(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate label 'Discord'"):
            parse_links(
                "Discord:\n  url: https://one.example\n  type:\n  - contact\n"
                "Discord:\n  url: https://two.example\n  type:\n  - contact\n"
            )

    def test_rejects_missing_url(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing url"):
            parse_links("Discord:\n  type:\n  - contact\n")

    def test_rejects_missing_type(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing type"):
            parse_links("Discord:\n  url: https://discord.example\n")

    def test_rejects_empty_type_list(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing type"):
            parse_links("Discord:\n  url: https://discord.example\n  type:\n")

    def test_rejects_inline_type_value(self) -> None:
        with self.assertRaisesRegex(ValueError, "type must be a list"):
            parse_links("Discord:\n  url: https://discord.example\n  type: contact\n")

    def test_rejects_unknown_keys(self) -> None:
        with self.assertRaisesRegex(ValueError, "unknown key 'badge'"):
            parse_links("Discord:\n  url: https://discord.example\n  badge: blue\n  type:\n  - contact\n")

    def test_rejects_a_non_http_url(self) -> None:
        with self.assertRaisesRegex(ValueError, "must be an HTTP\\(S\\) URL"):
            parse_links("Discord:\n  url: javascript:alert(1)\n  type:\n  - contact\n")

    def test_rejects_an_empty_document(self) -> None:
        with self.assertRaisesRegex(ValueError, "no links"):
            parse_links("# just a comment\n")

    def test_rejects_a_list_item_outside_type(self) -> None:
        with self.assertRaisesRegex(ValueError, "list item is not under type"):
            parse_links("Discord:\n  - contact\n  url: https://discord.example\n")

    def test_rejects_duplicate_type_tags(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate type tag 'contact'"):
            parse_links("Discord:\n  url: https://discord.example\n  type:\n  - contact\n  - contact\n")

    def test_rejects_malformed_labels_and_properties(self) -> None:
        invalid_sources = {
            "expected 'Label:'": "Discord\n",
            "indented line is not under a label": "  url: https://example.test\n",
            "expected 'key: value'": "Discord:\n  malformed\n",
            "empty url": "Discord:\n  url:\n  type:\n  - contact\n",
        }

        for message, source in invalid_sources.items():
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                parse_links(source, path="custom-links.yaml")

    def test_load_links_reads_an_explicit_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "links.yaml"
            path.write_text(SAMPLE, encoding="utf-8")

            links = load_links(path)

        self.assertEqual(["Discord", "Nexus Mods", "Docs"], [link.label for link in links])


class FilterAndRenderTests(unittest.TestCase):
    def test_filter_by_tag_keeps_yaml_order_without_naming_labels(self) -> None:
        contact = filter_by_tag(sample_links(), "contact")
        download = filter_by_tag(sample_links(), "download")

        self.assertEqual(["Discord", "Nexus Mods"], [link.label for link in contact])
        self.assertEqual(["Nexus Mods"], [link.label for link in download])
        self.assertEqual(sample_links(), filter_by_tag(sample_links(), None))

    def test_render_html_anchors_escapes_label_and_url(self) -> None:
        rendered = render_html_anchors([Link("A & B", 'https://example.test/?q=a&b="c"', frozenset({"contact"}))])

        self.assertIn("&" + "amp;", rendered)
        self.assertIn("&" + "quot;", rendered)
        self.assertNotIn("A & B", rendered)
        self.assertTrue(rendered.startswith("<a href="))
        self.assertTrue(rendered.endswith("</a>"))

    def test_render_html_list_items_wrap_anchors(self) -> None:
        rendered = render_html_list_items(filter_by_tag(sample_links(), "documentation"))

        self.assertEqual(
            '<li><a href="https://docs.example/" target="_blank" rel="noopener noreferrer">Docs</a></li>',
            rendered,
        )

    def test_render_bbcode_list_emits_a_nexus_list(self) -> None:
        rendered = render_bbcode_list(filter_by_tag(sample_links(), "documentation"))

        self.assertEqual("[list]\n[*][url=https://docs.example/]Docs[/url][/*]\n[/list]", rendered)

    def test_render_markdown_list_items_emits_linked_bullets(self) -> None:
        rendered = render_markdown_list_items(filter_by_tag(sample_links(), "contact"))

        self.assertEqual(
            "- [Discord](https://discord.example/invite)\n- [Nexus Mods](https://nexus.example/mod)",
            rendered,
        )

    def test_render_plain_text_pads_labels_to_the_longest(self) -> None:
        rendered = render_plain_text(filter_by_tag(sample_links(), "contact"))

        self.assertEqual(
            "  Discord     https://discord.example/invite\n  Nexus Mods  https://nexus.example/mod\n",
            rendered,
        )


class ReplaceMarkerTests(unittest.TestCase):
    def test_html_markers_filter_by_the_tag_in_the_marker(self) -> None:
        text = "start <!--CONTACT-LINKS--> mid <!--LINKS--> end"
        result = replace_html_link_markers(text, render_html_anchors, sample_links())

        self.assertIn("https://discord.example/invite", result.split("mid")[0])
        self.assertNotIn("https://docs.example/", result.split("mid")[0])
        self.assertIn("https://docs.example/", result.split("mid")[1])
        self.assertNotIn("CONTACT-LINKS", result)
        self.assertNotIn("<!--LINKS-->", result)

    def test_angle_markers_filter_by_the_tag_in_the_marker(self) -> None:
        result = replace_angle_link_markers(
            "Contact:\n<CONTACT-LINKS>",
            render_plain_text,
            sample_links(),
        )

        self.assertEqual(
            "Contact:\n  Discord     https://discord.example/invite\n  Nexus Mods  https://nexus.example/mod\n",
            result,
        )
        self.assertNotIn("https://docs.example/", result)

    def test_replace_is_a_noop_when_no_marker_is_present(self) -> None:
        self.assertEqual("unchanged", replace_html_link_markers("unchanged", render_html_anchors))

    def test_unknown_tag_in_a_marker_is_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "no links tagged 'forum'"):
            replace_html_link_markers("<!--FORUM-LINKS-->", render_html_anchors, sample_links())

    def test_empty_unfiltered_marker_is_an_error(self) -> None:
        with self.assertRaisesRegex(ValueError, "custom.yaml: no links"):
            replace_html_link_markers("<!--LINKS-->", render_html_anchors, [], Path("custom.yaml"))


if __name__ == "__main__":
    unittest.main()
