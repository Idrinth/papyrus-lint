#!/usr/bin/env python3
"""Unit tests for the Nexus page lint table generator."""

import importlib.util
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("generate_nexuspage_tables.py")
SPEC = importlib.util.spec_from_file_location("generate_nexuspage_tables", SCRIPT)
assert SPEC and SPEC.loader
generate_nexuspage_tables = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = generate_nexuspage_tables
SPEC.loader.exec_module(generate_nexuspage_tables)


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

        table = generate_nexuspage_tables.render_table(rules)

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
            generate_nexuspage_tables.render_table([]),
        )


class RenderTablesTests(unittest.TestCase):
    def test_groups_by_category_in_fixed_order(self) -> None:
        rules = [
            {"id": "b", "name": "B", "description": "b desc", "fixable": False, "category": "Other"},
            {"id": "a", "name": "A", "description": "a desc", "fixable": False, "category": "Formatting"},
        ]

        tables = generate_nexuspage_tables.render_tables(rules)

        self.assertEqual(len(generate_nexuspage_tables.CATEGORIES), len(tables))
        formatting_table = tables[generate_nexuspage_tables.CATEGORIES.index("Formatting")]
        other_table = tables[generate_nexuspage_tables.CATEGORIES.index("Other")]
        self.assertIn("[b]A[/b]", formatting_table)
        self.assertIn("[b]B[/b]", other_table)
        empty_table = tables[generate_nexuspage_tables.CATEGORIES.index("Performance")]
        self.assertNotIn("[tr][td]", empty_table)

    def test_rejects_an_unknown_category(self) -> None:
        rules = [{"id": "x", "name": "X", "description": "x desc", "fixable": False, "category": "Nope"}]

        with self.assertRaisesRegex(ValueError, "unknown category 'Nope'"):
            generate_nexuspage_tables.render_tables(rules)


class ApplyTests(unittest.TestCase):
    def test_replaces_each_table_block_in_order_and_leaves_the_rest_alone(self) -> None:
        text = wrap("[spoiler][table]old-first[/table][/spoiler]", "[spoiler][table]old-second[/table][/spoiler]")

        updated = generate_nexuspage_tables.apply(
            text, ["[spoiler][table]new-first[/table][/spoiler]", "[spoiler][table]new-second[/table][/spoiler]"]
        )

        self.assertEqual(
            wrap("[spoiler][table]new-first[/table][/spoiler]", "[spoiler][table]new-second[/table][/spoiler]"),
            updated,
        )
        self.assertIn("Intro paragraph.", updated)
        self.assertIn("Outro paragraph.", updated)

    def test_rejects_a_block_count_mismatch(self) -> None:
        text = wrap("[spoiler][table]only-one[/table][/spoiler]")

        with self.assertRaisesRegex(ValueError, r"expected 2 \[spoiler\]\[table\] blocks, found 1"):
            generate_nexuspage_tables.apply(text, ["a", "b"])


class MainTests(unittest.TestCase):
    def write_fixture(self, directory: str) -> tuple[Path, Path]:
        rules_path = Path(directory, "rules.json")
        rules_path.write_text(
            '[{"id": "a", "name": "A", "description": "a desc", "fixable": true, "category": "Formatting"}]',
            encoding="utf-8",
        )
        placeholder_blocks = [
            "[spoiler][table]stale[/table][/spoiler]" for _ in generate_nexuspage_tables.CATEGORIES
        ]
        bbcode_path = Path(directory, "page.bbcode")
        bbcode_path.write_text(wrap(*placeholder_blocks), encoding="utf-8")
        return rules_path, bbcode_path

    def test_check_fails_and_leaves_a_stale_file_untouched(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rules_path, bbcode_path = self.write_fixture(directory)
            stale = bbcode_path.read_text(encoding="utf-8")
            output = io.StringIO()

            with (
                mock.patch.object(
                    sys, "argv", ["generate_nexuspage_tables.py", "--check", str(rules_path), str(bbcode_path)]
                ),
                redirect_stdout(output),
            ):
                result = generate_nexuspage_tables.main()

            self.assertEqual(1, result)
            self.assertIn("out of date", output.getvalue())
            self.assertEqual(stale, bbcode_path.read_text(encoding="utf-8"))

    def test_writes_the_regenerated_file_without_check(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rules_path, bbcode_path = self.write_fixture(directory)
            output = io.StringIO()

            with (
                mock.patch.object(sys, "argv", ["generate_nexuspage_tables.py", str(rules_path), str(bbcode_path)]),
                redirect_stdout(output),
            ):
                result = generate_nexuspage_tables.main()

            self.assertEqual(0, result)
            self.assertIn("Regenerated lint tables", output.getvalue())
            self.assertIn("[b]A[/b]", bbcode_path.read_text(encoding="utf-8"))

    def test_check_passes_for_an_up_to_date_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rules_path, bbcode_path = self.write_fixture(directory)
            with mock.patch.object(sys, "argv", ["generate_nexuspage_tables.py", str(rules_path), str(bbcode_path)]):
                generate_nexuspage_tables.main()
            output = io.StringIO()

            with (
                mock.patch.object(
                    sys, "argv", ["generate_nexuspage_tables.py", "--check", str(rules_path), str(bbcode_path)]
                ),
                redirect_stdout(output),
            ):
                result = generate_nexuspage_tables.main()

            self.assertEqual(0, result)
            self.assertIn("up to date", output.getvalue())


if __name__ == "__main__":
    unittest.main()
