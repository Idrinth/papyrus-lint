#!/usr/bin/env python3
"""Unit tests for the Nexus page lint table generator's CLI entrypoint."""

import importlib.util
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import nexuspage_tables

SCRIPT = Path(__file__).with_name("generate_nexuspage_tables.py")
SPEC = importlib.util.spec_from_file_location("generate_nexuspage_tables", SCRIPT)
assert SPEC and SPEC.loader
generate_nexuspage_tables = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = generate_nexuspage_tables
SPEC.loader.exec_module(generate_nexuspage_tables)


def wrap(*blocks: str) -> str:
    """A minimal BBCode document carrying one [spoiler][table] block per
    entry of *blocks*, prefixed/interleaved with unrelated prose the way a
    real Nexus page does."""
    return "\n\n".join(["Intro paragraph.", *blocks, "Outro paragraph."])


class MainTests(unittest.TestCase):
    def write_fixture(self, directory: str) -> tuple[Path, Path]:
        rules_path = Path(directory, "rules.json")
        rules_path.write_text(
            '[{"id": "a", "name": "A", "description": "a desc", "fixable": true, "category": "Formatting"}]',
            encoding="utf-8",
        )
        placeholder_blocks = [
            "[spoiler][table][/table][/spoiler]" for _ in nexuspage_tables.CATEGORIES
        ]
        bbcode_path = Path(directory, "page.bbcode")
        bbcode_path.write_text(wrap(*placeholder_blocks), encoding="utf-8")
        return rules_path, bbcode_path

    def test_writes_the_regenerated_file(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rules_path, bbcode_path = self.write_fixture(directory)
            output = io.StringIO()

            with (
                mock.patch.object(sys, "argv", ["generate_nexuspage_tables.py", str(rules_path), str(bbcode_path)]),
                redirect_stdout(output),
            ):
                result = generate_nexuspage_tables.main()

            self.assertEqual(0, result)
            self.assertIn("Regenerated lint tables", output.getvalue())
            self.assertIn("[b]A[/b]", bbcode_path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
