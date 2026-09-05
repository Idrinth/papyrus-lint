#!/usr/bin/env python3
"""Unit tests for the dependency-free BBCode structural linter."""

import importlib.util
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock


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

    def test_reports_closing_tag_with_argument(self) -> None:
        self.assertEqual(
            ["closing tag [/url] cannot have an argument"],
            self.messages("[url=https://example.com]Example[/url=ignored]"),
        )

    def test_reports_closing_tag_without_an_opening_tag(self) -> None:
        self.assertEqual(
            ["closing tag [/b] has no opening tag"],
            self.messages("text[/b]"),
        )

    def test_plain_square_brackets_are_not_tags(self) -> None:
        self.assertEqual([], self.messages("values[0] and []"))

    def test_location_reports_one_based_line_and_column(self) -> None:
        text = "first line\nsecond [b]line"
        self.assertEqual((2, 8), bbcode_lint.location(text, text.index("[b]")))

    def test_check_file_prints_each_issue_with_its_location(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "page.bbcode")
            path.write_text("[b]ok[/b]\n[/i]\n[u]", encoding="utf-8")
            output = io.StringIO()

            with redirect_stdout(output):
                issue_count = bbcode_lint.check_file(path)

        self.assertEqual(2, issue_count)
        self.assertEqual(
            f"{path}:2:1: closing tag [/i] has no opening tag\n"
            f"{path}:3:1: tag [u] is not closed\n",
            output.getvalue(),
        )

    def test_main_reports_success_for_every_input_file(self) -> None:
        paths = [Path("one.bbcode"), Path("two.bbcode")]
        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["bbcode_lint.py", *map(str, paths)]),
            mock.patch.object(bbcode_lint, "check_file", side_effect=[0, 0]) as check_file,
            redirect_stdout(output),
        ):
            result = bbcode_lint.main()

        self.assertEqual(0, result)
        self.assertEqual([mock.call(path) for path in paths], check_file.call_args_list)
        self.assertEqual("BBCode lint passed for 2 file(s).\n", output.getvalue())

    def test_main_reports_the_total_failure_count(self) -> None:
        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["bbcode_lint.py", "one.bbcode", "two.bbcode"]),
            mock.patch.object(bbcode_lint, "check_file", side_effect=[1, 2]),
            redirect_stdout(output),
        ):
            result = bbcode_lint.main()

        self.assertEqual(1, result)
        self.assertEqual("BBCode lint failed with 3 issue(s).\n", output.getvalue())


if __name__ == "__main__":
    unittest.main()
