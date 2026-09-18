#!/usr/bin/env python3
"""Unit tests for the shared/rules.json assembly logic."""

import json
import tempfile
import unittest
from pathlib import Path

from ci_lib.rules_json import assemble_rules, render_rules_json


class AssembleRulesTests(unittest.TestCase):
    def write_rule(self, directory: str, file_stem: str, **fields: object) -> None:
        Path(directory, f"{file_stem}.json").write_text(json.dumps(fields), encoding="utf-8")

    def test_assembles_every_file_sorted_by_id(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.write_rule(directory, "zzz-rule", id="zzz-rule", name="Z")
            self.write_rule(directory, "aaa-rule", id="aaa-rule", name="A")

            rules = assemble_rules(Path(directory))

            self.assertEqual(["aaa-rule", "zzz-rule"], [rule["id"] for rule in rules])

    def test_rejects_a_rule_whose_id_does_not_match_its_file_name(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.write_rule(directory, "some-rule", id="other-id", name="Mismatched")

            with self.assertRaisesRegex(ValueError, "expected 'some-rule'"):
                assemble_rules(Path(directory))

    def test_rejects_an_empty_directory(self) -> None:
        with (
            tempfile.TemporaryDirectory() as directory,
            self.assertRaisesRegex(ValueError, "no rule files found"),
        ):
            assemble_rules(Path(directory))


class RenderRulesJsonTests(unittest.TestCase):
    def test_renders_an_indented_array_with_trailing_newline(self) -> None:
        rendered = render_rules_json([{"id": "a"}])

        self.assertEqual('[\n  {\n    "id": "a"\n  }\n]\n', rendered)


if __name__ == "__main__":
    unittest.main()
