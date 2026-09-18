#!/usr/bin/env python3
"""Unit tests for the semantic version advisory logic."""

import unittest

from ci_lib import semver


class ClassifyPullRequestTests(unittest.TestCase):
    def test_breaking_change_label_recommends_major(self) -> None:
        self.assertEqual("major", semver.classify_pull_request(["type: breaking change"]))

    def test_feature_label_recommends_minor(self) -> None:
        self.assertEqual("minor", semver.classify_pull_request(["type: feature"]))

    def test_refactoring_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["type: refactoring"]))

    def test_tests_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["type: tests"]))

    def test_documentation_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["type: documentation"]))

    def test_dependency_label_recommends_patch(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["type: dependency"]))

    def test_matching_is_case_insensitive(self) -> None:
        self.assertEqual("major", semver.classify_pull_request(["Type: Breaking Change"]))

    def test_unrelated_labels_recommend_nothing(self) -> None:
        self.assertIsNone(semver.classify_pull_request(["component: cli", "codex"]))

    def test_no_labels_recommends_nothing(self) -> None:
        self.assertIsNone(semver.classify_pull_request([]))

    def test_breaking_change_wins_over_feature_on_the_same_pull_request(self) -> None:
        self.assertEqual(
            "major",
            semver.classify_pull_request(["type: feature", "type: breaking change"]),
        )

    def test_feature_wins_over_a_patch_level_label_on_the_same_pull_request(self) -> None:
        self.assertEqual(
            "minor",
            semver.classify_pull_request(["type: tests", "type: feature"]),
        )

    def test_ci_only_component_recommends_patch_with_no_type_label(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["component: ci"]))

    def test_pages_only_component_recommends_patch_with_no_type_label(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["component: pages"]))

    def test_documentation_only_component_recommends_patch_with_no_type_label(self) -> None:
        self.assertEqual("patch", semver.classify_pull_request(["component: documentation"]))

    def test_mix_of_non_user_facing_components_recommends_patch(self) -> None:
        self.assertEqual(
            "patch",
            semver.classify_pull_request(["component: ci", "component: pages"]),
        )

    def test_non_user_facing_component_caps_a_breaking_change_label_at_patch(self) -> None:
        self.assertEqual(
            "patch",
            semver.classify_pull_request(["component: ci", "type: breaking change"]),
        )

    def test_non_user_facing_component_caps_a_feature_label_at_patch(self) -> None:
        self.assertEqual(
            "patch",
            semver.classify_pull_request(["component: documentation", "type: feature"]),
        )

    def test_user_facing_component_alongside_a_non_user_facing_one_is_not_capped(self) -> None:
        self.assertEqual(
            "major",
            semver.classify_pull_request(
                ["component: ci", "component: frontend", "type: breaking change"]
            ),
        )


class RecommendBumpTests(unittest.TestCase):
    def test_empty_list_recommends_nothing(self) -> None:
        self.assertIsNone(semver.recommend_bump([]))

    def test_no_recognized_labels_recommends_nothing(self) -> None:
        prs = [{"number": 1, "labels": ["component: cli"]}]
        self.assertIsNone(semver.recommend_bump(prs))

    def test_takes_the_highest_precedence_bump_across_pull_requests(self) -> None:
        prs = [
            {"number": 1, "labels": ["type: documentation"]},
            {"number": 2, "labels": ["type: feature"]},
            {"number": 3, "labels": ["type: tests"]},
        ]
        self.assertEqual("minor", semver.recommend_bump(prs))

    def test_breaking_change_beats_everything(self) -> None:
        prs = [
            {"number": 1, "labels": ["type: feature"]},
            {"number": 2, "labels": ["type: breaking change"]},
            {"number": 3, "labels": ["type: refactoring"]},
        ]
        self.assertEqual("major", semver.recommend_bump(prs))

    def test_only_patch_level_labels_recommends_patch(self) -> None:
        prs = [
            {"number": 1, "labels": ["type: refactoring"]},
            {"number": 2, "labels": ["type: tests"]},
        ]
        self.assertEqual("patch", semver.recommend_bump(prs))


class BumpVersionTests(unittest.TestCase):
    def test_major_bump_resets_minor_and_patch(self) -> None:
        self.assertEqual("v2.0.0", semver.bump_version("v1.5.9", "major"))

    def test_minor_bump_resets_patch(self) -> None:
        self.assertEqual("v1.6.0", semver.bump_version("v1.5.9", "minor"))

    def test_patch_bump_increments_patch_only(self) -> None:
        self.assertEqual("v1.5.10", semver.bump_version("v1.5.9", "patch"))

    def test_missing_tag_is_treated_as_v0_0_0(self) -> None:
        self.assertEqual("v1.0.0", semver.bump_version(None, "major"))
        self.assertEqual("v0.1.0", semver.bump_version(None, "minor"))
        self.assertEqual("v0.0.1", semver.bump_version(None, "patch"))

    def test_unrecognized_bump_level_is_rejected(self) -> None:
        with self.assertRaises(ValueError):
            semver.bump_version("v1.0.0", "unknown")


class DedupePullRequestsTests(unittest.TestCase):
    def test_keeps_the_first_entry_for_a_repeated_number(self) -> None:
        prs = [
            {"number": 1, "title": "First seen", "labels": []},
            {"number": 1, "title": "Duplicate", "labels": ["type: feature"]},
            {"number": 2, "title": "Other", "labels": []},
        ]
        deduped = semver.dedupe_pull_requests(prs)
        self.assertEqual([1, 2], [pr["number"] for pr in deduped])
        self.assertEqual("First seen", deduped[0]["title"])

    def test_skips_entries_with_no_number(self) -> None:
        prs = [{"title": "No number"}, {"number": 1, "title": "Has number"}]
        self.assertEqual([1], [pr["number"] for pr in semver.dedupe_pull_requests(prs)])


class BuildSummaryTests(unittest.TestCase):
    def test_reports_no_pull_requests_found(self) -> None:
        summary = semver.build_summary("v1.0.0", [], None, None)
        self.assertIn("No merged pull requests found since the latest release.", summary)
        self.assertIn("`v1.0.0`", summary)

    def test_labels_the_baseline_when_there_is_no_previous_release(self) -> None:
        summary = semver.build_summary(None, [], None, None)
        self.assertIn("(no previous release)", summary)

    def test_lists_each_pull_request_with_its_recommended_bump(self) -> None:
        prs = [
            {"number": 12, "title": "Add a feature", "labels": ["type: feature"]},
            {"number": 7, "title": "Fix typo", "labels": ["type: documentation"]},
        ]
        summary = semver.build_summary("v1.0.0", prs, "minor", "v1.1.0")
        self.assertIn("| #7 | Fix typo | patch |", summary)
        self.assertIn("| #12 | Add a feature | minor |", summary)
        # Sorted by PR number ascending regardless of input order.
        self.assertLess(summary.index("#7"), summary.index("#12"))

    def test_unlabeled_pull_request_is_shown_with_an_em_dash(self) -> None:
        prs = [{"number": 1, "title": "No type label", "labels": ["component: cli"]}]
        summary = semver.build_summary("v1.0.0", prs, None, None)
        self.assertIn("| #1 | No type label | — |", summary)
        self.assertIn("no version bump can be recommended", summary)

    def test_reports_the_recommended_next_version(self) -> None:
        prs = [{"number": 1, "title": "Breaking change", "labels": ["type: breaking change"]}]
        summary = semver.build_summary("v1.2.3", prs, "major", "v2.0.0")
        self.assertIn("**Recommended next version: `v2.0.0`** (major bump).", summary)


class BuildReleaseNotesTests(unittest.TestCase):
    def test_empty_when_no_bump_is_recommended(self) -> None:
        self.assertEqual("", semver.build_release_notes([], None, None))

    def test_includes_the_marker_and_bump_level(self) -> None:
        prs = [{"number": 5, "title": "Add a feature", "labels": ["type: feature"]}]
        notes = semver.build_release_notes(prs, "minor", "v1.1.0")
        self.assertIn(semver.RELEASE_MARKER, notes)
        self.assertIn("minor bump", notes)
        self.assertIn("- #5 Add a feature", notes)

    def test_lists_pull_requests_sorted_by_number(self) -> None:
        prs = [
            {"number": 12, "title": "Second", "labels": ["type: feature"]},
            {"number": 3, "title": "First", "labels": ["type: feature"]},
        ]
        notes = semver.build_release_notes(prs, "minor", "v1.1.0")
        self.assertLess(notes.index("#3"), notes.index("#12"))

    def test_includes_a_coverage_section_when_a_summary_is_given(self) -> None:
        prs = [{"number": 5, "title": "Add a feature", "labels": ["type: feature"]}]
        notes = semver.build_release_notes(
            prs, "minor", "v1.1.0", coverage_summary="| Module | Coverage |\n| --- | --- |\n"
        )
        self.assertIn("### Test coverage", notes)
        self.assertIn("| Module | Coverage |", notes)

    def test_omits_the_coverage_section_when_no_summary_is_given(self) -> None:
        prs = [{"number": 5, "title": "Add a feature", "labels": ["type: feature"]}]
        notes = semver.build_release_notes(prs, "minor", "v1.1.0")
        self.assertNotIn("### Test coverage", notes)

    def test_omits_the_coverage_section_when_the_summary_is_blank(self) -> None:
        prs = [{"number": 5, "title": "Add a feature", "labels": ["type: feature"]}]
        notes = semver.build_release_notes(prs, "minor", "v1.1.0", coverage_summary="   \n")
        self.assertNotIn("### Test coverage", notes)

    def test_escapes_a_mention_in_a_pull_request_title(self) -> None:
        prs = [{"number": 5, "title": "Thanks @octocat for the report", "labels": ["type: feature"]}]
        notes = semver.build_release_notes(prs, "minor", "v1.1.0")
        self.assertIn("- #5 Thanks `@octocat` for the report", notes)


class EscapeMentionsTests(unittest.TestCase):
    def test_wraps_a_plain_mention_in_backticks(self) -> None:
        self.assertEqual("fix by `@octocat`", semver.escape_mentions("fix by @octocat"))

    def test_wraps_a_team_mention_in_backticks(self) -> None:
        self.assertEqual(
            "cc `@idrinth/maintainers`", semver.escape_mentions("cc @idrinth/maintainers")
        )

    def test_leaves_text_with_no_mention_unchanged(self) -> None:
        self.assertEqual("no mentions here", semver.escape_mentions("no mentions here"))

    def test_wraps_every_mention_in_text_with_several(self) -> None:
        self.assertEqual(
            "`@alice` and `@bob`", semver.escape_mentions("@alice and @bob")
        )


if __name__ == "__main__":
    unittest.main()
