#!/usr/bin/env python3
"""Unit tests for the dependency-free BBCode structural linter."""

import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("bbcode_lint.py")
SPEC = importlib.util.spec_from_file_location("bbcode_lint", SCRIPT)
assert SPEC and SPEC.loader
bbcode_lint = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = bbcode_lint
SPEC.loader.exec_module(bbcode_lint)


class BbcodeLintTests(unittest.TestCase):
    def messages(self, text: str) -> list[str]:
        return [issue.message for issue in bbcode_lint.lint(text)]

    def test_accepts_nested_case_insensitive_tags_and_arguments(self) -> None:
        self.assertEqual([], self.messages("[SIZE=4][b]Title[/B][/size]"))

    def test_accepts_nexus_list_items(self) -> None:
        self.assertEqual([], self.messages("[list][*]One[/*][*]Two[/*][/list]"))

    def test_reports_unclosed_tag(self) -> None:
        self.assertEqual(["tag [b] is not closed"], self.messages("[b]text"))

    def test_reports_misnested_tags(self) -> None:
        self.assertEqual(
            [
                "tag [i] is not closed",
                "closing tag [/i] does not match open [b]",
            ],
            self.messages("[i][b]text[/i][/b]"),
        )

    def test_reports_unknown_tag(self) -> None:
        self.assertEqual(
            ["unknown tag [haeding]", "unknown tag [haeding]"],
            self.messages("[haeding]Title[/haeding]"),
        )

    def test_plain_square_brackets_are_not_tags(self) -> None:
        self.assertEqual([], self.messages("values[0] and []"))


if __name__ == "__main__":
    unittest.main()
