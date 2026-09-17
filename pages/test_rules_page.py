"""Tests for the rules.html subpage."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from pages import rules_page


class RulesPageTest(unittest.TestCase):
    def test_build_rules_page_renders_table_and_filters(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            rules_file = root / "rules.json"
            rules_file.write_text(
                json.dumps(
                    [
                        {
                            "id": "example-rule",
                            "name": "Example rule",
                            "tags": ["correctness", "style"],
                            "severity": "warning",
                            "fixable": True,
                            "description": "Flags an `example`.",
                            "definition": "The full behavior of this rule.",
                        }
                    ]
                ),
                encoding="utf-8",
            )
            (pages_dir / "rules.template.html").write_text(
                "<title>Lint rules</title><main><!--RULES_CONTENT--></main>", encoding="utf-8"
            )

            with (
                patch.object(rules_page, "PAGES_DIR", pages_dir),
                patch.object(rules_page, "RULES_FILE", rules_file),
            ):
                rules_page.build_rules_page(out_dir, version="v1.0.0")

            output = (out_dir / "rules.html").read_text(encoding="utf-8")

        self.assertNotIn("<!--RULES_CONTENT-->", output)
        self.assertIn('id="rule-example-rule"', output)
        self.assertIn("Example rule", output)
        self.assertIn('<code>example-rule</code>', output)
        self.assertIn('data-severity="warning"', output)
        self.assertIn('data-tags="correctness style"', output)
        self.assertIn('data-fixable="true"', output)
        self.assertIn('<td class="fix-yes">✓</td>', output)
        self.assertIn("<code>example</code>", output)
        self.assertIn("The full behavior of this rule.", output)
        self.assertIn('value="warning"', output)
        self.assertIn('value="correctness"', output)
        self.assertIn('value="style"', output)
        self.assertNotIn('value="performance"', output)
        self.assertNotIn('value="maintainability"', output)
        self.assertNotIn('value="error"', output)
        self.assertNotIn('value="info"', output)
        self.assertIn('id="rules-fixable-filter"', output)
        self.assertIn('id="rules-count"', output)

    def test_build_rules_page_rejects_a_template_missing_a_marker(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pages_dir = root / "pages"
            out_dir = root / "out"
            pages_dir.mkdir()
            out_dir.mkdir()
            rules_file = root / "rules.json"
            rules_file.write_text("[]", encoding="utf-8")
            (pages_dir / "rules.template.html").write_text("<main>No marker</main>", encoding="utf-8")

            with (
                patch.object(rules_page, "PAGES_DIR", pages_dir),
                patch.object(rules_page, "RULES_FILE", rules_file),
                self.assertRaisesRegex(SystemExit, "missing marker"),
            ):
                rules_page.build_rules_page(out_dir)

            self.assertFalse((out_dir / "rules.html").exists())


class RepositoryRulesConfigurationTest(unittest.TestCase):
    """Keep rules_page.py's checked-in inputs synchronized with docs/rules.json."""

    def test_rules_json_has_unique_ids_and_known_severities_and_tags(self) -> None:
        rules = rules_page.load_rules()

        self.assertTrue(rules)
        ids = [rule["id"] for rule in rules]
        self.assertEqual(len(ids), len(set(ids)), "rule ids must be unique")
        for rule in rules:
            with self.subTest(rule=rule["id"]):
                self.assertIn(rule["severity"], rules_page.RULE_SEVERITIES)
                self.assertTrue(rule["tags"])
                for tag in rule["tags"]:
                    self.assertIn(tag, rules_page.RULE_TAGS)
                self.assertIsInstance(rule["fixable"], bool)
                self.assertTrue(rule["name"].strip())
                self.assertTrue(rule["description"].strip())
                self.assertTrue(rule["definition"].strip())


if __name__ == "__main__":
    unittest.main()
