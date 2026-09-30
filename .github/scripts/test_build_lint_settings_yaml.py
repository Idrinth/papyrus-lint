#!/usr/bin/env python3
"""Unit tests for the lint-settings YAML builder's CLI entrypoint."""

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("build_lint_settings_yaml.py")
SPEC = importlib.util.spec_from_file_location("build_lint_settings_yaml", SCRIPT)
assert SPEC and SPEC.loader
build_lint_settings_yaml = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = build_lint_settings_yaml
SPEC.loader.exec_module(build_lint_settings_yaml)


class MainTests(unittest.TestCase):
    def test_writes_the_combined_yaml(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            project.joinpath("compiler_path.json").write_text(
                json.dumps({"key": "compiler_path", "yaml": {"default": "null", "comment": "Path"}}),
                encoding="utf-8",
            )
            lint.joinpath("game.json").write_text(
                json.dumps({"key": "game", "yaml": {"default": "skyrim", "comment": "Game"}}),
                encoding="utf-8",
            )
            meta = root / "lint-settings.yaml"
            meta.write_text(
                "rules_comment: Each rule accepts true or false\n"
                "project:\n"
                "  - compiler_path\n"
                "settings:\n"
                "  - game\n",
                encoding="utf-8",
            )
            out_path = root / "lint-settings.generated.yaml"
            output = io.StringIO()

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "build_lint_settings_yaml.py",
                        "--project-settings-dir",
                        str(project),
                        "--lint-settings-dir",
                        str(lint),
                        "--meta",
                        str(meta),
                        "--out",
                        str(out_path),
                    ],
                ),
                redirect_stdout(output),
            ):
                result = build_lint_settings_yaml.main()

            self.assertEqual(0, result)
            self.assertIn("Wrote 1 project settings and 1 lint settings", output.getvalue())
            self.assertIn("key: compiler_path", out_path.read_text(encoding="utf-8"))
            self.assertIn("key: game", out_path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
