#!/usr/bin/env python3
"""Unit tests for the semantic version advisory script."""

import importlib.util
import json
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


semver_advisory = load_script("semver_advisory")


class ClassifyPullRequestTests(unittest.TestCase):
    def test_breaking_change_label_recommends_major(self) -> None:
        self.assertEqual("major", semver_advisory.classify_pull_request(["type: breaking change"]))

    def test_feature_label_recommends_minor(self) -> None:
        self.assertEqual("minor", semver_advisory.classify_pull_request(["type: feature"]))

    def test_refactoring_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver_advisory.classify_pull_request(["type: refactoring"]))

    def test_tests_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver_advisory.classify_pull_request(["type: tests"]))

    def test_documentation_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver_advisory.classify_pull_request(["type: documentation"]))

    def test_matching_is_case_insensitive(self) -> None:
        self.assertEqual("major", semver_advisory.classify_pull_request(["Type: Breaking Change"]))

    def test_unrelated_labels_recommend_nothing(self) -> None:
        self.assertIsNone(semver_advisory.classify_pull_request(["component: cli", "codex"]))

    def test_no_labels_recommends_nothing(self) -> None:
        self.assertIsNone(semver_advisory.classify_pull_request([]))

    def test_breaking_change_wins_over_feature_on_the_same_pull_request(self) -> None:
        self.assertEqual(
            "major",
            semver_advisory.classify_pull_request(["type: feature", "type: breaking change"]),
        )

    def test_feature_wins_over_a_patch_level_label_on_the_same_pull_request(self) -> None:
        self.assertEqual(
            "minor",
            semver_advisory.classify_pull_request(["type: tests", "type: feature"]),
        )


class RecommendBumpTests(unittest.TestCase):
    def test_empty_list_recommends_nothing(self) -> None:
        self.assertIsNone(semver_advisory.recommend_bump([]))

    def test_no_recognized_labels_recommends_nothing(self) -> None:
        prs = [{"number": 1, "labels": ["component: cli"]}]
        self.assertIsNone(semver_advisory.recommend_bump(prs))

    def test_takes_the_highest_precedence_bump_across_pull_requests(self) -> None:
        prs = [
            {"number": 1, "labels": ["type: documentation"]},
            {"number": 2, "labels": ["type: feature"]},
            {"number": 3, "labels": ["type: tests"]},
        ]
        self.assertEqual("minor", semver_advisory.recommend_bump(prs))

    def test_breaking_change_beats_everything(self) -> None:
        prs = [
            {"number": 1, "labels": ["type: feature"]},
            {"number": 2, "labels": ["type: breaking change"]},
            {"number": 3, "labels": ["type: refactoring"]},
        ]
        self.assertEqual("major", semver_advisory.recommend_bump(prs))

    def test_only_patch_level_labels_recommends_patch(self) -> None:
        prs = [
            {"number": 1, "labels": ["type: refactoring"]},
            {"number": 2, "labels": ["type: tests"]},
        ]
        self.assertEqual("patch", semver_advisory.recommend_bump(prs))


class BumpVersionTests(unittest.TestCase):
    def test_major_bump_resets_minor_and_patch(self) -> None:
        self.assertEqual("v2.0.0", semver_advisory.bump_version("v1.5.9", "major"))

    def test_minor_bump_resets_patch(self) -> None:
        self.assertEqual("v1.6.0", semver_advisory.bump_version("v1.5.9", "minor"))

    def test_patch_bump_increments_patch_only(self) -> None:
        self.assertEqual("v1.5.10", semver_advisory.bump_version("v1.5.9", "patch"))

    def test_missing_tag_is_treated_as_v0_0_0(self) -> None:
        self.assertEqual("v1.0.0", semver_advisory.bump_version(None, "major"))
        self.assertEqual("v0.1.0", semver_advisory.bump_version(None, "minor"))
        self.assertEqual("v0.0.1", semver_advisory.bump_version(None, "patch"))

    def test_unrecognized_bump_level_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            semver_advisory.bump_version("v1.0.0", "unknown")


class DedupePullRequestsTests(unittest.TestCase):
    def test_keeps_the_first_entry_for_a_repeated_number(self) -> None:
        prs = [
            {"number": 1, "title": "First seen", "labels": []},
            {"number": 1, "title": "Duplicate", "labels": ["type: feature"]},
            {"number": 2, "title": "Other", "labels": []},
        ]
        deduped = semver_advisory.dedupe_pull_requests(prs)
        self.assertEqual([1, 2], [pr["number"] for pr in deduped])
        self.assertEqual("First seen", deduped[0]["title"])

    def test_skips_entries_with_no_number(self) -> None:
        prs = [{"title": "No number"}, {"number": 1, "title": "Has number"}]
        self.assertEqual([1], [pr["number"] for pr in semver_advisory.dedupe_pull_requests(prs)])


class BuildSummaryTests(unittest.TestCase):
    def test_reports_no_pull_requests_found(self) -> None:
        summary = semver_advisory.build_summary("v1.0.0", [], None, None)
        self.assertIn("No merged pull requests found since the latest release.", summary)
        self.assertIn("`v1.0.0`", summary)

    def test_labels_the_baseline_when_there_is_no_previous_release(self) -> None:
        summary = semver_advisory.build_summary(None, [], None, None)
        self.assertIn("(no previous release)", summary)

    def test_lists_each_pull_request_with_its_recommended_bump(self) -> None:
        prs = [
            {"number": 12, "title": "Add a feature", "labels": ["type: feature"]},
            {"number": 7, "title": "Fix typo", "labels": ["type: documentation"]},
        ]
        summary = semver_advisory.build_summary("v1.0.0", prs, "minor", "v1.1.0")
        self.assertIn("| #7 | Fix typo | patch |", summary)
        self.assertIn("| #12 | Add a feature | minor |", summary)
        # Sorted by PR number ascending regardless of input order.
        self.assertLess(summary.index("#7"), summary.index("#12"))

    def test_unlabeled_pull_request_is_shown_with_an_em_dash(self) -> None:
        prs = [{"number": 1, "title": "No type label", "labels": ["component: cli"]}]
        summary = semver_advisory.build_summary("v1.0.0", prs, None, None)
        self.assertIn("| #1 | No type label | — |", summary)
        self.assertIn("no version bump can be recommended", summary)

    def test_reports_the_recommended_next_version(self) -> None:
        prs = [{"number": 1, "title": "Breaking change", "labels": ["type: breaking change"]}]
        summary = semver_advisory.build_summary("v1.2.3", prs, "major", "v2.0.0")
        self.assertIn("**Recommended next version: `v2.0.0`** (major bump).", summary)


class MainTests(unittest.TestCase):
    def test_main_prints_a_recommendation_for_the_given_pull_requests(self) -> None:
        import contextlib
        import io
        from unittest import mock

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
        import contextlib
        import io
        from unittest import mock

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


if __name__ == "__main__":
    unittest.main()
