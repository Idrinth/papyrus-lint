#!/usr/bin/env python3
"""Tests for default config + schema generation from lint-settings + rules."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import yaml

from ci_lib.config_schema import render_schema
from ci_lib.default_config import (
    alphabetical_rule_keys,
    config_key_for,
    load_lint_settings,
    load_rules,
    render_default_yaml,
    write_default_yaml,
)

ROOT = Path(__file__).resolve().parents[2]


def _nested_game_setting() -> dict:
    return {
        "key": "game",
        "yaml": {"default": "skyrim", "comment": "game"},
    }


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

    def test_load_rules_prefers_combined_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "shared").mkdir()
            expected = [{"id": "combined"}]
            (root / "shared/rules.json").write_text(json.dumps(expected), encoding="utf-8")
            self.assertEqual(expected, load_rules(root))

    def test_load_rules_validates_split_file_names(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            rules_dir = root / "shared" / "rules"
            rules_dir.mkdir(parents=True)
            rules_dir.joinpath("file-name.json").write_text(
                json.dumps({"id": "different-id"}), encoding="utf-8"
            )
            with self.assertRaisesRegex(ValueError, "id 'different-id' != 'file-name'"):
                load_rules(root)

    def test_write_default_yaml_creates_default_parent_directory(self) -> None:
        settings = {
            "project_settings": [],
            "settings": [_nested_game_setting()],
            "rules_yaml_comment": "rules",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config_dir = root / "shared" / "configuration"
            rules_dir = root / "shared" / "rules"
            config_dir.mkdir(parents=True)
            rules_dir.mkdir(parents=True)
            config_dir.joinpath("lint-settings.yaml").write_text(
                yaml.safe_dump(settings, sort_keys=False), encoding="utf-8"
            )
            rules_dir.joinpath("line-length.json").write_text(
                json.dumps({"id": "line-length"}), encoding="utf-8"
            )

            destination = write_default_yaml(root)

            self.assertEqual(config_dir / "papyrus-lint.default.yaml", destination)
            self.assertIn("  line_length: true", destination.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
