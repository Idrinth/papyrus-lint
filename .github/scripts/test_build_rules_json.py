#!/usr/bin/env python3
"""Unit tests for the shared/rules.json builder's CLI entrypoint."""

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).with_name("build_rules_json.py")
SPEC = importlib.util.spec_from_file_location("build_rules_json", SCRIPT)
assert SPEC and SPEC.loader
build_rules_json = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = build_rules_json
SPEC.loader.exec_module(build_rules_json)


class MainTests(unittest.TestCase):
    def test_writes_the_combined_rules_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            rules_dir = Path(directory, "rules")
            rules_dir.mkdir()
            rules_dir.joinpath("a-rule.json").write_text(
                json.dumps({"id": "a-rule", "name": "A"}), encoding="utf-8"
            )
            out_path = Path(directory, "rules.json")
            output = io.StringIO()

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    ["build_rules_json.py", "--rules-dir", str(rules_dir), "--out", str(out_path)],
                ),
                redirect_stdout(output),
            ):
                result = build_rules_json.main()

            self.assertEqual(0, result)
            self.assertIn("Wrote 1 rules", output.getvalue())
            self.assertEqual([{"id": "a-rule", "name": "A"}], json.loads(out_path.read_text(encoding="utf-8")))


if __name__ == "__main__":
    unittest.main()
