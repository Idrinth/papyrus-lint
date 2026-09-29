#!/usr/bin/env python3
"""Tests for default config + schema generation from lint-settings + rules."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

from ci_lib.config_schema import render_schema
from ci_lib.default_config import (
    alphabetical_rule_keys,
    config_key_for,
    load_lint_settings,
    render_default_yaml,
)

ROOT = Path(__file__).resolve().parents[2]


class DefaultConfigTests(unittest.TestCase):
    def test_config_key_overrides(self) -> None:
        self.assertEqual(config_key_for("float-to-int"), "float_int_conversion")
        self.assertEqual(config_key_for("too-many-named-states"), "too_many_states")
        self.assertEqual(config_key_for("line-length"), "line_length")

    def test_render_default_yaml_matches_lint_settings_and_rules(self) -> None:
        settings = load_lint_settings(ROOT)
        rules = [
            json.loads(path.read_text(encoding="utf-8"))
            for path in sorted((ROOT / "shared/rules").glob("*.json"))
        ]
        text = render_default_yaml(settings, rules)
        self.assertTrue(text.startswith("# game"))
        self.assertIn("\ngame: skyrim\n", text)
        self.assertIn("\ncompiler_path: null\n", text)
        self.assertIn("\nrules:\n", text)
        self.assertIn("  member_chain_none_usage: false\n", text)
        self.assertEqual(text.count("\nrules:\n"), 1)
        # Rules are alphabetical by config key.
        rules_block = text.split("\nrules:\n", 1)[1].strip().splitlines()
        keys = [line.split(":", 1)[0].strip() for line in rules_block if line.strip()]
        self.assertEqual(keys, sorted(keys))
        self.assertEqual(keys[0], "argument_naming")

    def test_render_schema_covers_games_and_all_rules(self) -> None:
        settings = load_lint_settings(ROOT)
        rules = [
            json.loads(path.read_text(encoding="utf-8"))
            for path in sorted((ROOT / "shared/rules").glob("*.json"))
        ]
        schema = render_schema(settings, rules)
        self.assertEqual(
            schema["properties"]["game"]["enum"], ["skyrim", "fallout4", "starfield"]
        )
        self.assertIn("max_line_length", schema["properties"])
        self.assertEqual(len(schema["properties"]["rules"]["properties"]), len(rules))
        rule_keys = list(schema["properties"]["rules"]["properties"])
        self.assertEqual(rule_keys, sorted(rule_keys))
        for key in (
            "deprecated_functions",
            "final_newline",
            "get_form_from_file_load_index",
            "line_length",
            "member_access",
            "place_at_me",
            "undefined_event",
            "unused_state",
            "member_chain_none_usage",
            "register_for_update_in_on_update",
        ):
            self.assertIn(key, schema["properties"]["rules"]["properties"])

    def test_duplicate_rule_config_key_raises(self) -> None:
        with self.assertRaisesRegex(ValueError, "duplicate rule config key"):
            alphabetical_rule_keys(
                [
                    {"id": "line-length"},
                    {"id": "line-length"},
                ]
            )


if __name__ == "__main__":
    unittest.main()
