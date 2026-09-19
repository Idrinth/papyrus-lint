#!/usr/bin/env python3
"""Unit tests for the frontend config-types.ts generator."""

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from ci_lib.config_types import config_key_for, render_config_types

SCRIPT = Path(__file__).with_name("generate_config_types.py")
SPEC = importlib.util.spec_from_file_location("generate_config_types", SCRIPT)
assert SPEC and SPEC.loader
generate_config_types = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = generate_config_types
SPEC.loader.exec_module(generate_config_types)

MINIMAL_TOP = """\
semicolon: false
indentation: tab
indentation_width: 4
identifier_casing: PascalCase
cyclomatic_complexity_warning: 10
cyclomatic_complexity_error: 20
type_casing: PascalCase
named_arguments: never
min_wait_interval: 0.1
magic_numbers: loose
fail_on_warning: false
fail_on_info: false
bool_like_int: true
assume_auto_properties_filled: false
"""


def yaml_for(*rule_lines: str) -> str:
    return MINIMAL_TOP + "rules:\n" + "".join(f"  {line}\n" for line in rule_lines)


class ConfigKeyTests(unittest.TestCase):
    def test_replaces_hyphens_except_for_known_overrides(self) -> None:
        self.assertEqual("trailing_whitespace", config_key_for("trailing-whitespace"))
        self.assertEqual("float_int_conversion", config_key_for("float-to-int"))
        self.assertEqual("too_many_states", config_key_for("too-many-named-states"))


class RenderConfigTypesTests(unittest.TestCase):
    def test_renders_rules_in_yaml_order_with_enabled_by_default(self) -> None:
        rendered = render_config_types(
            [
                {"id": "comma-spacing", "enabled_by_default": True},
                {"id": "property-sorting", "enabled_by_default": False},
            ],
            yaml_for("comma_spacing: true", "property_sorting: false"),
        )

        self.assertIn(
            "export interface LintRules {\n  comma_spacing: boolean;\n  property_sorting: boolean;\n}",
            rendered,
        )
        self.assertIn("  comma_spacing: true,\n  property_sorting: false,\n", rendered)
        self.assertIn('indentation: "tab"', rendered)
        self.assertIn("bool_like_int: true,", rendered)
        self.assertIn("Do not edit by hand.", rendered)

    def test_maps_float_to_int_onto_float_int_conversion(self) -> None:
        rendered = render_config_types(
            [{"id": "float-to-int", "enabled_by_default": True}],
            yaml_for("float_int_conversion: true"),
        )

        self.assertIn("  float_int_conversion: boolean;", rendered)
        self.assertNotIn("float_to_int", rendered)

    def test_rejects_a_yaml_rule_missing_from_shared_rules(self) -> None:
        with self.assertRaisesRegex(ValueError, "no matching id"):
            render_config_types(
                [{"id": "comma-spacing", "enabled_by_default": True}],
                yaml_for("comma_spacing: true", "missing_rule: true"),
            )

    def test_rejects_a_shared_rule_missing_from_the_yaml(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing rules: unused_property"):
            render_config_types(
                [
                    {"id": "comma-spacing", "enabled_by_default": True},
                    {"id": "unused-property", "enabled_by_default": True},
                ],
                yaml_for("comma_spacing: true"),
            )

    def test_rejects_yaml_default_disagreeing_with_enabled_by_default(self) -> None:
        with self.assertRaisesRegex(ValueError, "enabled_by_default"):
            render_config_types(
                [{"id": "comma-spacing", "enabled_by_default": True}],
                yaml_for("comma_spacing: false"),
            )


class MainTests(unittest.TestCase):
    def test_writes_the_generated_typescript(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rules_dir = Path(directory, "rules")
            rules_dir.mkdir()
            rules_dir.joinpath("comma-spacing.json").write_text(
                json.dumps({"id": "comma-spacing", "enabled_by_default": True}),
                encoding="utf-8",
            )
            default_yaml = Path(directory, "default.yaml")
            default_yaml.write_text(yaml_for("comma_spacing: true"), encoding="utf-8")
            out_path = Path(directory, "config-types.ts")
            output = io.StringIO()

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "generate_config_types.py",
                        "--rules-dir",
                        str(rules_dir),
                        "--default-yaml",
                        str(default_yaml),
                        "--out",
                        str(out_path),
                    ],
                ),
                redirect_stdout(output),
            ):
                result = generate_config_types.main()

            self.assertEqual(0, result)
            self.assertIn("Wrote 1 rule flags", output.getvalue())
            written = out_path.read_text(encoding="utf-8")
            self.assertIn("comma_spacing: boolean;", written)
            self.assertIn("export function setCurrentLintConfig", written)


if __name__ == "__main__":
    unittest.main()
