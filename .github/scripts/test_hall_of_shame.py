#!/usr/bin/env python3
"""Unit tests for the hall-of-shame CLI entrypoint."""

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path

from ci_lib import source_metrics


def load_script(name: str):
    path = Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


hall_of_shame = load_script("hall_of_shame")


class MainTests(unittest.TestCase):
    def test_main_writes_markdown_and_notice_annotations(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app").mkdir()
            big = root / "app" / "big.rs"
            big.write_text("pub fn one() {}\n" * 20 + "pub struct Two;\n" * 5, encoding="utf-8")
            small = root / "app" / "small.rs"
            small.write_text("fn hidden() {}\n", encoding="utf-8")
            reports = root / "cov"
            reports.mkdir()
            (reports / "lcov.info").write_text(
                f"SF:{big.as_posix()}\nLF:25\nLH:1\nend_of_record\n",
                encoding="utf-8",
            )

            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                contextlib.redirect_stdout(stdout),
                contextlib.redirect_stderr(stderr),
            ):
                hall_of_shame.main(["--root", str(root), "--lcov-dir", str(reports), "--top", "3"])

            output = stdout.getvalue()
            notices = stderr.getvalue()
            self.assertIn(source_metrics.MARKER, output)
            self.assertIn("### Hall of shame", output)
            self.assertIn("`app/big.rs`", output)
            self.assertIn("::notice title=Hall of shame — size::", notices)
            self.assertIn("::notice title=Hall of shame — exports::", notices)
            self.assertIn("::notice title=Hall of shame — uncovered lines::", notices)
            self.assertIn("::notice title=Hall of shame — LOC::", notices)
            self.assertIn("app/big.rs", notices)

    def test_main_without_lcov_dir_notes_missing_reports(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "only.py").write_text("def public():\n    return 1\n", encoding="utf-8")
            stdout = io.StringIO()
            stderr = io.StringIO()
            with (
                contextlib.redirect_stdout(stdout),
                contextlib.redirect_stderr(stderr),
            ):
                hall_of_shame.main(["--root", str(root), "--no-notices"])

            self.assertIn("_no coverage reports_", stdout.getvalue())
            self.assertEqual("", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
