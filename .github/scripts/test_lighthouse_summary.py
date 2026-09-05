#!/usr/bin/env python3
"""Unit tests for the Lighthouse report summary script."""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


def load_script(name: str):
    path = Path(__file__).with_name(f"{name}.py")
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


lighthouse_summary = load_script("lighthouse_summary")


def make_report(url: str, scores: dict, failing_audits: dict | None = None) -> dict:
    """Builds a minimal Lighthouse-report-shaped dict: `scores` maps category
    ids (e.g. "performance") to a 0-1 score, `failing_audits` maps a category
    id to a list of (audit_id, audit_score, audit_title) tuples referenced by
    that category with nonzero weight."""
    failing_audits = failing_audits or {}
    categories = {}
    audits = {}
    for key, score in scores.items():
        audit_refs = []
        for audit_id, audit_score, title in failing_audits.get(key, []):
            audit_refs.append({"id": audit_id, "weight": 1})
            audits[audit_id] = {"score": audit_score, "title": title}
        categories[key] = {"score": score, "auditRefs": audit_refs}
    return {"finalUrl": url, "categories": categories, "audits": audits}


class PageLabelTests(unittest.TestCase):
    def test_uses_final_url_path(self) -> None:
        report = {"finalUrl": "http://localhost:4173/docs/index.html"}
        self.assertEqual("/docs/index.html", lighthouse_summary.page_label(report))

    def test_falls_back_to_requested_url(self) -> None:
        report = {"requestedUrl": "http://localhost:4173/videos.html"}
        self.assertEqual("/videos.html", lighthouse_summary.page_label(report))

    def test_root_path_when_url_has_no_path(self) -> None:
        report = {"finalUrl": "http://localhost:4173"}
        self.assertEqual("/", lighthouse_summary.page_label(report))


class FormatScoreTests(unittest.TestCase):
    def test_formats_a_passing_score_without_a_flag(self) -> None:
        self.assertEqual("95", lighthouse_summary.format_score(0.95))

    def test_flags_a_score_below_threshold(self) -> None:
        self.assertEqual("80 ⚠️", lighthouse_summary.format_score(0.8))

    def test_reports_n_a_for_a_missing_score(self) -> None:
        self.assertEqual("n/a", lighthouse_summary.format_score(None))


class FailingAuditsTests(unittest.TestCase):
    def test_ignores_categories_at_or_above_threshold(self) -> None:
        report = make_report(
            "http://x/index.html",
            {"performance": 1.0},
            {"performance": [("unused-audit", 0.2, "Unused audit")]},
        )
        self.assertEqual([], lighthouse_summary.failing_audits(report))

    def test_lists_titles_for_a_failing_category_weakest_first(self) -> None:
        report = make_report(
            "http://x/index.html",
            {"performance": 0.5},
            {
                "performance": [
                    ("uses-optimized-images", 0.4, "Efficiently encode images"),
                    ("render-blocking-resources", 0.1, "Eliminate render-blocking resources"),
                ]
            },
        )
        self.assertEqual(
            ["Eliminate render-blocking resources", "Efficiently encode images"],
            lighthouse_summary.failing_audits(report),
        )

    def test_ignores_zero_weight_audit_refs(self) -> None:
        report = {
            "categories": {
                "seo": {
                    "score": 0.5,
                    "auditRefs": [{"id": "manual-check", "weight": 0}],
                }
            },
            "audits": {"manual-check": {"score": 0.0, "title": "Manual check"}},
        }
        self.assertEqual([], lighthouse_summary.failing_audits(report))


class BuildSummaryTests(unittest.TestCase):
    def test_reports_no_reports_generated(self) -> None:
        summary = lighthouse_summary.build_summary([])
        self.assertIn(lighthouse_summary.MARKER, summary)
        self.assertIn("No Lighthouse reports were generated.", summary)

    def test_reports_all_pages_passing_when_nothing_fails(self) -> None:
        reports = [
            make_report(
                "http://x/index.html",
                {"performance": 1.0, "accessibility": 1.0, "best-practices": 1.0, "seo": 1.0},
            )
        ]
        summary = lighthouse_summary.build_summary(reports)
        self.assertIn("| /index.html | 100 | 100 | 100 | 100 |", summary)
        self.assertIn("All pages scored at least 90/100 in every category.", summary)

    def test_flags_a_low_score_and_lists_its_audits_sorted_by_page(self) -> None:
        reports = [
            make_report(
                "http://x/videos.html",
                {"performance": 1.0, "accessibility": 1.0, "best-practices": 1.0, "seo": 1.0},
            ),
            make_report(
                "http://x/index.html",
                {"performance": 0.7, "accessibility": 1.0, "best-practices": 1.0, "seo": 1.0},
                {"performance": [("render-blocking-resources", 0.2, "Eliminate render-blocking resources")]},
            ),
        ]
        summary = lighthouse_summary.build_summary(reports)

        self.assertIn("| /index.html | 70 ⚠️ | 100 | 100 | 100 |", summary)
        self.assertIn("| /videos.html | 100 | 100 | 100 | 100 |", summary)
        self.assertIn("**/index.html**", summary)
        self.assertIn("Eliminate render-blocking resources", summary)
        self.assertNotIn("**/videos.html**", summary)
        index_pos = summary.index("| /index.html")
        videos_pos = summary.index("| /videos.html")
        self.assertLess(index_pos, videos_pos)

    def test_uses_the_default_marker_and_title_when_no_label_is_given(self) -> None:
        summary = lighthouse_summary.build_summary([])
        self.assertIn(lighthouse_summary.MARKER, summary)
        self.assertIn("### Lighthouse report", summary)

    def test_accepts_a_custom_marker_and_title(self) -> None:
        summary = lighthouse_summary.build_summary(
            [], marker="<!-- custom-marker -->", title="Custom report"
        )
        self.assertIn("<!-- custom-marker -->", summary)
        self.assertIn("### Custom report", summary)
        self.assertNotIn(lighthouse_summary.MARKER, summary)


class LoadReportsTests(unittest.TestCase):
    def test_returns_empty_list_for_a_missing_directory(self) -> None:
        self.assertEqual([], lighthouse_summary.load_reports(Path("does-not-exist")))

    def test_loads_only_report_json_files(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index__html.report.json").write_text('{"finalUrl": "http://x/index.html"}', encoding="utf-8")
            (root / "index__html.report.html").write_text("<html></html>", encoding="utf-8")

            reports = lighthouse_summary.load_reports(root)

        self.assertEqual(1, len(reports))
        self.assertEqual("http://x/index.html", reports[0]["finalUrl"])


class MainTests(unittest.TestCase):
    def test_main_uses_the_default_marker_when_no_label_is_given(self) -> None:
        import contextlib
        import io
        from unittest import mock

        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["lighthouse_summary.py", "does-not-exist"]),
            contextlib.redirect_stdout(output),
        ):
            lighthouse_summary.main()

        self.assertIn(lighthouse_summary.MARKER, output.getvalue())
        self.assertIn("### Lighthouse report", output.getvalue())

    def test_main_builds_a_labelled_marker_and_title_when_a_label_is_given(self) -> None:
        import contextlib
        import io
        from unittest import mock

        output = io.StringIO()
        with (
            mock.patch.object(sys, "argv", ["lighthouse_summary.py", "does-not-exist", "App"]),
            contextlib.redirect_stdout(output),
        ):
            lighthouse_summary.main()

        rendered = output.getvalue()
        self.assertIn("<!-- lighthouse-summary-comment-App -->", rendered)
        self.assertIn("### App Lighthouse report", rendered)
        self.assertNotIn(lighthouse_summary.MARKER, rendered)


if __name__ == "__main__":
    unittest.main()
