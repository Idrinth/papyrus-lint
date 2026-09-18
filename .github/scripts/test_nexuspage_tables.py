#!/usr/bin/env python3
"""Unit tests for the Nexus page lint table generation logic."""

import unittest

from ci_lib import nexuspage_tables


def wrap(*blocks: str) -> str:
    """A minimal BBCode document carrying one [spoiler][table] block per
    entry of *blocks*, prefixed/interleaved with unrelated prose the way a
    real Nexus page does."""
    return "\n\n".join(["Intro paragraph.", *blocks, "Outro paragraph."])


class RenderTableTests(unittest.TestCase):
    def test_renders_header_and_one_row_per_rule(self) -> None:
        rules = [
            {"name": "Trailing whitespace", "description": "Flags trailing spaces.", "fixable": True},
            {"name": "Magic numbers", "description": "Flags bare literals.", "fixable": False},
        ]

        table = nexuspage_tables.render_table(rules)

        self.assertEqual(
            "[spoiler][table]"
            "[tr][th]Lint[/th]\n[th]Description[/th]\n[th]Auto-Fix[/th]\n[/tr]\n"
            "[tr][td][b]Trailing whitespace[/b][/td]\n"
            "[td]Flags trailing spaces.[/td]\n"
            "[td]✓[/td]\n"
            "[/tr]\n"
            "[tr][td][b]Magic numbers[/b][/td]\n"
            "[td]Flags bare literals.[/td]\n"
            "[td][/td]\n"
            "[/tr]\n"
            "[/table][/spoiler]",
            table,
        )

    def test_renders_header_only_for_an_empty_category(self) -> None:
        self.assertEqual(
            "[spoiler][table][tr][th]Lint[/th]\n[th]Description[/th]\n[th]Auto-Fix[/th]\n[/tr]\n[/table][/spoiler]",
            nexuspage_tables.render_table([]),
        )


class RenderTablesTests(unittest.TestCase):
    def test_groups_by_category_in_fixed_order(self) -> None:
        rules = [
            {"id": "b", "name": "B", "description": "b desc", "fixable": False, "category": "Other"},
            {"id": "a", "name": "A", "description": "a desc", "fixable": False, "category": "Formatting"},
        ]

        tables = nexuspage_tables.render_tables(rules)

        self.assertEqual(len(nexuspage_tables.CATEGORIES), len(tables))
        formatting_table = tables[nexuspage_tables.CATEGORIES.index("Formatting")]
        other_table = tables[nexuspage_tables.CATEGORIES.index("Other")]
        self.assertIn("[b]A[/b]", formatting_table)
        self.assertIn("[b]B[/b]", other_table)
        empty_table = tables[nexuspage_tables.CATEGORIES.index("Performance")]
        self.assertNotIn("[tr][td]", empty_table)

    def test_rejects_an_unknown_category(self) -> None:
        rules = [{"id": "x", "name": "X", "description": "x desc", "fixable": False, "category": "Nope"}]

        with self.assertRaisesRegex(ValueError, "unknown category 'Nope'"):
            nexuspage_tables.render_tables(rules)


class ApplyTests(unittest.TestCase):
    def test_replaces_each_table_block_in_order_and_leaves_the_rest_alone(self) -> None:
        text = wrap("[spoiler][table][/table][/spoiler]", "[spoiler][table][/table][/spoiler]")

        updated = nexuspage_tables.apply(
            text, ["[spoiler][table]new-first[/table][/spoiler]", "[spoiler][table]new-second[/table][/spoiler]"]
        )

        self.assertEqual(
            wrap("[spoiler][table]new-first[/table][/spoiler]", "[spoiler][table]new-second[/table][/spoiler]"),
            updated,
        )
        self.assertIn("Intro paragraph.", updated)
        self.assertIn("Outro paragraph.", updated)

    def test_rejects_a_block_count_mismatch(self) -> None:
        text = wrap("[spoiler][table][/table][/spoiler]")

        with self.assertRaisesRegex(ValueError, r"expected 2 \[spoiler\]\[table\] blocks, found 1"):
            nexuspage_tables.apply(text, ["a", "b"])


if __name__ == "__main__":
    unittest.main()
