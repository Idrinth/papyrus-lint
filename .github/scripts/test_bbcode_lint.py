#!/usr/bin/env python3
"""Unit tests for the BBCode linter's CLI entrypoint."""

import importlib.util
import io
import sys
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


class MainTests(unittest.TestCase):
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
