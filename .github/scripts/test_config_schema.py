#!/usr/bin/env python3
"""Unit tests for configuration schema generation and its CLI entrypoint."""

from __future__ import annotations

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import yaml

from ci_lib.config_schema import SCHEMA_ID, render_schema, write_schema

SCRIPT = Path(__file__).with_name("generate_config_schema.py")
SPEC = importlib.util.spec_from_file_location("generate_config_schema", SCRIPT)
assert SPEC and SPEC.loader
generate_config_schema = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = generate_config_schema
SPEC.loader.exec_module(generate_config_schema)


def fixture() -> tuple[dict, list[dict]]:
    settings = {
        "project_settings": [
            {
                "key": "compiler_path",
                "yaml": {"comment": "Compiler\npath", "default": "null"},
                "schema": {"type": ["string", "null"]},
            }
        ],
        "settings": [
            {
                "key": "game",
                "yaml": {"comment": "Target game", "default": "skyrim"},
                "schema": {
                    "type": "string",
                    "description": "Explicit description",
                    "description_extra": "Not part of JSON Schema",
                },
            },
            {
                "key": "threads",
                "doc": "Worker   thread count",
                "schema": {"type": "integer"},
            },
        ],
        "rules_yaml_comment": "rules",
    }
    rules = [
        {"id": "first-rule", "name": "First rule", "enabled_by_default": False},
        {"id": "second-rule", "description": "Second description"},
    ]
    return settings, rules


def write_inputs(root: Path) -> None:
    settings, rules = fixture()
    config_dir = root / "shared" / "configuration"
    rules_dir = root / "shared" / "rules"
    config_dir.mkdir(parents=True)
    rules_dir.mkdir(parents=True)
    config_dir.joinpath("lint-settings.yaml").write_text(
        yaml.safe_dump(settings, sort_keys=False), encoding="utf-8"
    )
    for rule in rules:
        rules_dir.joinpath(f"{rule['id']}.json").write_text(json.dumps(rule), encoding="utf-8")


class ConfigSchemaTests(unittest.TestCase):
    def test_render_schema_builds_settings_and_rule_switches(self) -> None:
        settings, rules = fixture()

        schema = render_schema(settings, rules)

        self.assertEqual(SCHEMA_ID, schema["$id"])
        self.assertFalse(schema["additionalProperties"])
        self.assertEqual("Explicit description", schema["properties"]["game"]["description"])
        self.assertNotIn("description_extra", schema["properties"]["game"])
        self.assertEqual("Compiler path", schema["properties"]["compiler_path"]["description"])
        self.assertEqual("Worker thread count", schema["properties"]["threads"]["description"])
        rule_properties = schema["properties"]["rules"]["properties"]
        # Alphabetical by config key: first_rule then second_rule
        self.assertEqual(["first_rule", "second_rule"], list(rule_properties))
        self.assertEqual(False, rule_properties["first_rule"]["default"])
        self.assertEqual("First rule", rule_properties["first_rule"]["description"])
        self.assertEqual(True, rule_properties["second_rule"]["default"])
        self.assertEqual("second_rule", rule_properties["second_rule"]["description"])

    def test_render_schema_orders_rules_alphabetically(self) -> None:
        settings, _ = fixture()
        rules = [
            {"id": "zeta-rule", "name": "Zeta"},
            {"id": "alpha-rule", "name": "Alpha", "enabled_by_default": False},
        ]
        schema = render_schema(settings, rules)
        self.assertEqual(
            ["alpha_rule", "zeta_rule"],
            list(schema["properties"]["rules"]["properties"]),
        )

    def test_write_schema_loads_sources_and_uses_requested_destination(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_inputs(root)
            destination = root / "generated.json"

            result = write_schema(root, destination)

            self.assertEqual(destination, result)
            self.assertEqual(render_schema(*fixture()), json.loads(destination.read_text(encoding="utf-8")))
            self.assertTrue(destination.read_text(encoding="utf-8").endswith("\n"))


class ConfigSchemaMainTests(unittest.TestCase):
    def test_main_writes_custom_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_inputs(root)
            output_path = root / "schema.json"
            output = io.StringIO()
            with (
                mock.patch.object(
                    sys,
                    "argv",
                    ["generate_config_schema.py", "--repo-root", str(root), "-o", str(output_path)],
                ),
                redirect_stdout(output),
            ):
                result = generate_config_schema.main()

            self.assertEqual(0, result)
            self.assertEqual(f"Wrote {output_path}\n", output.getvalue())
            self.assertTrue(output_path.is_file())

    def test_check_reports_matching_and_stale_schema(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_inputs(root)
            output_path = root / "schema.json"
            write_schema(root, output_path)
            argv = [
                "generate_config_schema.py",
                "--repo-root",
                str(root),
                "--check",
                "--output",
                str(output_path),
            ]
            output = io.StringIO()
            with mock.patch.object(sys, "argv", argv), redirect_stdout(output):
                self.assertEqual(0, generate_config_schema.main())
            self.assertIn("matches lint-settings + rules", output.getvalue())

            output_path.write_text("{}", encoding="utf-8")
            error = io.StringIO()
            with mock.patch.object(sys, "argv", argv), redirect_stderr(error):
                self.assertEqual(1, generate_config_schema.main())
            self.assertIn("is out of date", error.getvalue())

    def test_check_allows_missing_on_disk_schema(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_inputs(root)
            missing = root / "schema" / "papyrus-lint.schema.json"
            argv = [
                "generate_config_schema.py",
                "--repo-root",
                str(root),
                "--check",
                "--output",
                str(missing),
            ]
            output = io.StringIO()
            with mock.patch.object(sys, "argv", argv), redirect_stdout(output):
                self.assertEqual(0, generate_config_schema.main())
            self.assertIn("no on-disk copy", output.getvalue())


if __name__ == "__main__":
    unittest.main()
