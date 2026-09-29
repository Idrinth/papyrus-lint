#!/usr/bin/env python3
"""Unit tests for the lint-settings YAML assembler."""

import json
import tempfile
import unittest
from pathlib import Path

from ci_lib.lint_settings_yaml import assemble_lint_settings, render_lint_settings_yaml

REPO_ROOT = Path(__file__).resolve().parents[2]


class AssembleLintSettingsTests(unittest.TestCase):
    def write_setting(self, directory: Path, key: str, **fields: object) -> None:
        payload = {"key": key, **fields}
        directory.joinpath(f"{key}.json").write_text(json.dumps(payload), encoding="utf-8")

    def write_meta(self, directory: Path, **fields: object) -> Path:
        path = directory / "lint-settings.meta.json"
        path.write_text(json.dumps(fields), encoding="utf-8")
        return path

    def test_assembles_in_meta_order_not_filename_order(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            self.write_setting(project, "second")
            self.write_setting(project, "first")
            self.write_setting(lint, "zeta")
            self.write_setting(lint, "alpha")
            meta = self.write_meta(
                root,
                rules_yaml_comment="Each rule accepts true or false",
                project_settings=["first", "second"],
                settings=["zeta", "alpha"],
            )

            document = assemble_lint_settings(project, lint, meta)

            self.assertEqual(["first", "second"], [item["key"] for item in document["project_settings"]])
            self.assertEqual(["zeta", "alpha"], [item["key"] for item in document["settings"]])
            self.assertEqual("Each rule accepts true or false", document["rules_yaml_comment"])

    def test_rejects_a_key_that_does_not_match_its_file_name(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            project.joinpath("compiler_path.json").write_text(
                json.dumps({"key": "other"}), encoding="utf-8"
            )
            self.write_setting(lint, "game")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["compiler_path"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "expected 'compiler_path'"):
                assemble_lint_settings(project, lint, meta)

    def test_rejects_a_file_missing_from_the_meta_list(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            self.write_setting(project, "compiler_path")
            self.write_setting(project, "extra")
            self.write_setting(lint, "game")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["compiler_path"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "extra.json"):
                assemble_lint_settings(project, lint, meta)

    def test_rejects_a_listed_key_with_no_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            self.write_setting(project, "compiler_path")
            self.write_setting(lint, "game")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["compiler_path", "missing"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "missing"):
                assemble_lint_settings(project, lint, meta)

    def test_rejects_an_empty_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            self.write_setting(lint, "game")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["compiler_path"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "no setting files found"):
                assemble_lint_settings(project, lint, meta)

    def test_rejects_a_key_listed_in_both_sections(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            self.write_setting(project, "game")
            self.write_setting(lint, "game")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["game"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "both project_settings and settings"):
                assemble_lint_settings(project, lint, meta)

    def test_rejects_a_missing_meta_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            with self.assertRaisesRegex(ValueError, "does not exist"):
                assemble_lint_settings(root / "project", root / "lint", root / "missing.json")

    def test_rejects_invalid_meta_documents(self) -> None:
        invalid_documents = (
            ([], "must contain a JSON object"),
            ({"rules_yaml_comment": None}, "`rules_yaml_comment` must be a string"),
            (
                {"rules_yaml_comment": "rules", "project_settings": [], "settings": ["game"]},
                "`project_settings` must be a non-empty list",
            ),
            (
                {"rules_yaml_comment": "rules", "project_settings": [1], "settings": ["game"]},
                "`project_settings` entries must be non-empty strings",
            ),
            (
                {
                    "rules_yaml_comment": "rules",
                    "project_settings": ["path", "path"],
                    "settings": ["game"],
                },
                "`project_settings` lists a key more than once",
            ),
        )
        for document, message in invalid_documents:
            with self.subTest(document=document), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                meta = root / "meta.json"
                meta.write_text(json.dumps(document), encoding="utf-8")

                with self.assertRaisesRegex(ValueError, message):
                    assemble_lint_settings(root / "project", root / "lint", meta)

    def test_rejects_a_missing_settings_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            project.mkdir()
            self.write_setting(project, "compiler_path")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["compiler_path"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "no setting files found"):
                assemble_lint_settings(project, root / "missing", meta)

    def test_rejects_a_setting_that_is_not_an_object(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project-settings"
            lint = root / "lint-settings"
            project.mkdir()
            lint.mkdir()
            project.joinpath("compiler_path.json").write_text("[]", encoding="utf-8")
            self.write_setting(lint, "game")
            meta = self.write_meta(
                root,
                rules_yaml_comment="rules",
                project_settings=["compiler_path"],
                settings=["game"],
            )

            with self.assertRaisesRegex(ValueError, "expected a JSON object"):
                assemble_lint_settings(project, lint, meta)


class RenderLintSettingsYamlTests(unittest.TestCase):
    def test_renders_the_checked_in_quoting_style(self) -> None:
        rendered = render_lint_settings_yaml(
            {
                "project_settings": [
                    {
                        "key": "compiler_path",
                        "yaml": {"default": "null", "comment": "Path, or null\nto auto-detect"},
                        "schema": {"type": ["string", "null"], "default": None},
                    }
                ],
                "rules_yaml_comment": "Each rule accepts true or false",
                "settings": [
                    {
                        "key": "semicolon",
                        "rust": {"type": "bool", "default": "false"},
                        "ts_type": '"tab" | "space"',
                        "doc": 'See the "Semicolon" lint.',
                        "schema": {"type": "boolean", "default": False, "minimum": 0},
                    }
                ],
            },
            header=False,
        )

        self.assertEqual(
            "\n".join(
                [
                    "project_settings:",
                    "  - key: compiler_path",
                    "    yaml:",
                    "      default: 'null'",
                    '      comment: "Path, or null\\nto auto-detect"',
                    "    schema:",
                    "      type:",
                    "        - string",
                    "        - 'null'",
                    "      default: null",
                    "rules_yaml_comment: Each rule accepts true or false",
                    "settings:",
                    "  - key: semicolon",
                    "    rust:",
                    "      type: bool",
                    "      default: 'false'",
                    "    ts_type: '\"tab\" | \"space\"'",
                    '    doc: "See the \\"Semicolon\\" lint."',
                    "    schema:",
                    "      type: boolean",
                    "      default: false",
                    "      minimum: 0",
                    "",
                ]
            ),
            rendered,
        )

    def test_repo_sources_round_trip_through_yaml(self) -> None:
        import yaml

        configuration = REPO_ROOT / "shared" / "configuration"
        document = assemble_lint_settings(
            configuration / "project-settings",
            configuration / "lint-settings",
            configuration / "lint-settings.meta.json",
        )

        loaded = yaml.safe_load(render_lint_settings_yaml(document))

        self.assertEqual(document, loaded)

    def test_renders_empty_collections_numbers_and_quoted_keys(self) -> None:
        rendered = render_lint_settings_yaml(
            {
                "project_settings": [{}],
                "rules_yaml_comment": "rules",
                "settings": [
                    {
                        "key": "example",
                        "empty_list": [],
                        "empty_mapping": {},
                        "float": 1.25,
                        "not plain": "'quoted",
                    }
                ],
            },
            header=False,
        )

        self.assertIn("  - {}\n", rendered)
        self.assertIn("    empty_list: []\n", rendered)
        self.assertIn("    empty_mapping: {}\n", rendered)
        self.assertIn("    float: 1.25\n", rendered)
        self.assertIn('    "not plain": "\'quoted"\n', rendered)

    def test_quotes_strings_that_yaml_would_parse_as_other_types(self) -> None:
        import yaml

        values = ["", " padded ", "yes", "~", "12", "[]", "-", "#comment", "a: b", "a#b"]
        document = {
            "project_settings": [{"key": "project"}],
            "rules_yaml_comment": "rules",
            "settings": [{"key": "example", "values": values}],
        }

        loaded = yaml.safe_load(render_lint_settings_yaml(document, header=False))

        self.assertEqual(values, loaded["settings"][0]["values"])

    def test_rejects_unsupported_nested_values(self) -> None:
        documents = (
            {"unsupported": object()},
            {"items": [["nested"]]},
        )
        for setting in documents:
            with self.subTest(setting=setting), self.assertRaisesRegex(TypeError, "cannot render"):
                render_lint_settings_yaml(
                    {
                        "project_settings": [{"key": "project"}],
                        "rules_yaml_comment": "rules",
                        "settings": [setting],
                    },
                    header=False,
                )


if __name__ == "__main__":
    unittest.main()
