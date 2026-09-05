#!/usr/bin/env python3
"""Unit tests for the Lighthouse summary script."""

import contextlib
import importlib.util
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


def load_script(name: str):
    path = Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


lighthouse_summary = load_script("lighthouse_summary")


def make_report(scores: dict[str, float]) -> dict:
    """Builds a minimal Lighthouse report with one audit per category, each
    scored per the given category-key -> score mapping."""
    categories = {}
    audits = {}
    for category_key, score in scores.items():
        audit_id = f"{category_key}-audit"
        categories[category_key] = {"auditRefs": [{"id": audit_id, "weight": 1}]}
        audits[audit_id] = {"score": score, "title": f"{category_key} title"}
    return {"categories": categories, "audits": audits}


class ScoreStrTests(unittest.TestCase):
    def test_formats_fractional_score_as_percentage(self) -> None:
        self.assertEqual("90", lighthouse_summary.score_str(0.9))
        self.assertEqual("100", lighthouse_summary.score_str(1))

    def test_returns_na_for_non_numeric_score(self) -> None:
        self.assertEqual("n/a", lighthouse_summary.score_str(None))
        self.assertEqual("n/a", lighthouse_summary.score_str("n/a"))


class LoadJsonTests(unittest.TestCase):
    def test_returns_none_for_missing_file(self) -> None:
        self.assertIsNone(lighthouse_summary.load_json(Path("does-not-exist.json")))

    def test_returns_none_for_invalid_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "bad.json")
            path.write_text("not json", encoding="utf-8")
            self.assertIsNone(lighthouse_summary.load_json(path))

    def test_parses_valid_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "good.json")
            path.write_text(json.dumps({"a": 1}), encoding="utf-8")
            self.assertEqual({"a": 1}, lighthouse_summary.load_json(path))


class FailingAuditsTests(unittest.TestCase):
    def test_flags_audits_below_threshold(self) -> None:
        report = make_report({"performance": 0.5, "accessibility": 1.0})
        self.assertEqual(["Performance: performance title"], lighthouse_summary.failing_audits(report))

    def test_ignores_zero_weight_and_unscored_audits(self) -> None:
        report = {
            "categories": {
                "performance": {
                    "auditRefs": [
                        {"id": "zero-weight", "weight": 0},
                        {"id": "unscored", "weight": 1},
                        {"id": "missing-audit", "weight": 1},
                    ]
                }
            },
            "audits": {
                "zero-weight": {"score": 0.1, "title": "should be skipped (zero weight)"},
                "unscored": {"score": None, "title": "manual/informational audit"},
            },
        }
        self.assertEqual([], lighthouse_summary.failing_audits(report))

    def test_ignores_categories_not_in_the_reported_set(self) -> None:
        report = make_report({"pwa": 0.1})
        self.assertEqual([], lighthouse_summary.failing_audits(report))


class BuildSummaryTests(unittest.TestCase):
    def test_reports_no_report_when_manifest_is_empty(self) -> None:
        summary = lighthouse_summary.build_summary([])
        self.assertIn(lighthouse_summary.MARKER, summary)
        self.assertIn("did not produce a report", summary)

    def test_prefers_representative_runs_when_present(self) -> None:
        manifest = [
            {"url": "http://x/", "isRepresentativeRun": False, "summary": {"performance": 0.1}},
            {"url": "http://x/", "isRepresentativeRun": True, "summary": {"performance": 0.9}},
        ]
        summary = lighthouse_summary.build_summary(manifest)
        self.assertIn("| http://x/ | 90 | n/a | n/a | n/a |", summary)

    def test_falls_back_to_every_entry_when_none_marked_representative(self) -> None:
        manifest = [{"url": "http://x/", "summary": {"performance": 0.42}}]
        summary = lighthouse_summary.build_summary(manifest)
        self.assertIn("| http://x/ | 42 | n/a | n/a | n/a |", summary)

    def test_lists_failing_audits_from_the_full_report_on_disk(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report_path = Path(directory, "lhr.json")
            report_path.write_text(
                json.dumps(make_report({"performance": 0.3})), encoding="utf-8"
            )
            manifest = [
                {
                    "url": "http://x/",
                    "isRepresentativeRun": True,
                    "summary": {"performance": 0.3},
                    "jsonPath": str(report_path),
                }
            ]
            summary = lighthouse_summary.build_summary(manifest)

        self.assertIn("#### Audits below 90", summary)
        self.assertIn("- http://x/", summary)
        self.assertIn("Performance: performance title", summary)

    def test_reports_no_issues_when_json_report_is_missing_or_unreadable(self) -> None:
        manifest = [
            {
                "url": "http://x/",
                "isRepresentativeRun": True,
                "summary": {"performance": 0.5},
                "jsonPath": "does-not-exist.json",
            }
        ]
        summary = lighthouse_summary.build_summary(manifest)
        self.assertIn("No audits scored below the reporting threshold.", summary)

    def test_reports_no_issues_when_every_scored_audit_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            report_path = Path(directory, "lhr.json")
            report_path.write_text(
                json.dumps(make_report({"performance": 1.0})), encoding="utf-8"
            )
            manifest = [
                {
                    "url": "http://x/",
                    "isRepresentativeRun": True,
                    "summary": {"performance": 1.0},
                    "jsonPath": str(report_path),
                }
            ]
            summary = lighthouse_summary.build_summary(manifest)

        self.assertIn("No audits scored below the reporting threshold.", summary)


class MainTests(unittest.TestCase):
    def test_main_prints_summary_for_missing_manifest(self) -> None:
        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["lighthouse_summary.py", "does-not-exist.json"]),
            contextlib.redirect_stdout(output),
        ):
            lighthouse_summary.main()

        self.assertIn(lighthouse_summary.MARKER, output.getvalue())
        self.assertIn("did not produce a report", output.getvalue())


if __name__ == "__main__":
    unittest.main()
