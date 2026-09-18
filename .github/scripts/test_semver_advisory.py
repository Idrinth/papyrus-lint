#!/usr/bin/env python3
"""Unit tests for the semantic version advisory CLI entrypoint."""

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import semver

SCRIPT = Path(__file__).with_name("semver_advisory.py")
SPEC = importlib.util.spec_from_file_location("semver_advisory", SCRIPT)
assert SPEC and SPEC.loader
semver_advisory = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = semver_advisory
SPEC.loader.exec_module(semver_advisory)


class MainTests(unittest.TestCase):
    def test_main_prints_a_recommendation_for_the_given_pull_requests(self) -> None:
        prs = [{"number": 3, "title": "Add a feature", "labels": ["type: feature"]}]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "prs.json"
            path.write_text(json.dumps(prs), encoding="utf-8")

            output = io.StringIO()
            with (
                mock.patch.object(sys, "argv", ["semver_advisory.py", str(path), "v1.0.0"]),
                contextlib.redirect_stdout(output),
            ):
                semver_advisory.main()

        rendered = output.getvalue()
        self.assertIn("`v1.0.0`", rendered)
        self.assertIn("**Recommended next version: `v1.1.0`** (minor bump).", rendered)

    def test_main_treats_a_missing_tag_argument_as_no_previous_release(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "prs.json"
            path.write_text("[]", encoding="utf-8")

            output = io.StringIO()
            with (
                mock.patch.object(sys, "argv", ["semver_advisory.py", str(path)]),
                contextlib.redirect_stdout(output),
            ):
                semver_advisory.main()

        self.assertIn("(no previous release)", output.getvalue())

    def test_main_writes_release_notes_and_outputs_when_requested(self) -> None:
        prs = [{"number": 3, "title": "Add a feature", "labels": ["type: feature"]}]
        with tempfile.TemporaryDirectory() as directory:
            prs_path = Path(directory) / "prs.json"
            prs_path.write_text(json.dumps(prs), encoding="utf-8")
            notes_path = Path(directory) / "notes.md"
            outputs_path = Path(directory) / "outputs.json"

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "semver_advisory.py",
                        str(prs_path),
                        "v1.0.0",
                        "--release-notes",
                        str(notes_path),
                        "--outputs",
                        str(outputs_path),
                    ],
                ),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                semver_advisory.main()

            self.assertIn(semver.RELEASE_MARKER, notes_path.read_text(encoding="utf-8"))
            self.assertEqual(
                {"bump": "minor", "next_version": "v1.1.0"},
                json.loads(outputs_path.read_text(encoding="utf-8")),
            )

    def test_main_folds_a_coverage_summary_file_into_the_release_notes(self) -> None:
        prs = [{"number": 3, "title": "Add a feature", "labels": ["type: feature"]}]
        with tempfile.TemporaryDirectory() as directory:
            prs_path = Path(directory) / "prs.json"
            prs_path.write_text(json.dumps(prs), encoding="utf-8")
            notes_path = Path(directory) / "notes.md"
            coverage_path = Path(directory) / "coverage.md"
            coverage_path.write_text("| Module | Coverage |\n| --- | --- |\n", encoding="utf-8")

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "semver_advisory.py",
                        str(prs_path),
                        "v1.0.0",
                        "--release-notes",
                        str(notes_path),
                        "--coverage-summary",
                        str(coverage_path),
                    ],
                ),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                semver_advisory.main()

            notes = notes_path.read_text(encoding="utf-8")
            self.assertIn("### Test coverage", notes)
            self.assertIn("| Module | Coverage |", notes)

    def test_main_ignores_a_missing_coverage_summary_file(self) -> None:
        prs = [{"number": 3, "title": "Add a feature", "labels": ["type: feature"]}]
        with tempfile.TemporaryDirectory() as directory:
            prs_path = Path(directory) / "prs.json"
            prs_path.write_text(json.dumps(prs), encoding="utf-8")
            notes_path = Path(directory) / "notes.md"

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "semver_advisory.py",
                        str(prs_path),
                        "v1.0.0",
                        "--release-notes",
                        str(notes_path),
                        "--coverage-summary",
                        str(Path(directory) / "missing.md"),
                    ],
                ),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                semver_advisory.main()

            self.assertNotIn("### Test coverage", notes_path.read_text(encoding="utf-8"))

    def test_main_writes_empty_release_notes_and_null_outputs_when_no_bump(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            prs_path = Path(directory) / "prs.json"
            prs_path.write_text("[]", encoding="utf-8")
            notes_path = Path(directory) / "notes.md"
            outputs_path = Path(directory) / "outputs.json"

            with (
                mock.patch.object(
                    sys,
                    "argv",
                    [
                        "semver_advisory.py",
                        str(prs_path),
                        "--release-notes",
                        str(notes_path),
                        "--outputs",
                        str(outputs_path),
                    ],
                ),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                semver_advisory.main()

            self.assertEqual("", notes_path.read_text(encoding="utf-8"))
            self.assertEqual(
                {"bump": None, "next_version": None},
                json.loads(outputs_path.read_text(encoding="utf-8")),
            )


if __name__ == "__main__":
    unittest.main()
