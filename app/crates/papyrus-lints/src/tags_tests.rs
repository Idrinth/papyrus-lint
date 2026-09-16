use super::*;
use crate::KNOWN_RULE_IDS;
use std::collections::HashSet;

#[test]
fn every_known_rule_id_has_tags() {
    for rule in KNOWN_RULE_IDS {
        assert!(
            tags_for(rule).is_some(),
            "{rule:?} is in KNOWN_RULE_IDS but has no RULE_TAGS entry"
        );
    }
}

#[test]
fn every_tagged_rule_is_a_known_rule_id() {
    for tags in RULE_TAGS {
        assert!(
            KNOWN_RULE_IDS.contains(&tags.rule),
            "{:?} has a RULE_TAGS entry but isn't in KNOWN_RULE_IDS",
            tags.rule
        );
    }
}

#[test]
fn rule_tags_has_no_duplicate_rule_ids() {
    let mut seen = HashSet::new();
    for tags in RULE_TAGS {
        assert!(
            seen.insert(tags.rule),
            "{:?} appears more than once in RULE_TAGS",
            tags.rule
        );
    }
}

#[test]
fn every_rule_has_at_least_one_kind() {
    for tags in RULE_TAGS {
        assert!(
            !tags.kinds.is_empty(),
            "{:?} has no kind keywords",
            tags.rule
        );
    }
}

#[test]
fn every_rule_has_a_description() {
    for tags in RULE_TAGS {
        assert!(
            !tags.description.is_empty(),
            "{:?} has no description",
            tags.rule
        );
    }
}

#[test]
fn auto_fixable_matches_fixable_rule_ids() {
    for tags in RULE_TAGS {
        assert_eq!(
            tags.auto_fixable(),
            FIXABLE_RULE_IDS.contains(&tags.rule),
            "{:?}.auto_fixable() disagrees with FIXABLE_RULE_IDS",
            tags.rule
        );
    }
}

#[test]
fn tags_for_matches_case_insensitively() {
    assert_eq!(
        tags_for("TRAILING-WHITESPACE").map(|t| t.rule),
        Some(crate::trailing_whitespace::RULE)
    );
}

#[test]
fn tags_for_returns_none_for_an_unknown_rule() {
    assert!(tags_for("made-up-rule").is_none());
}

#[test]
fn every_rule_has_a_doc_slug_shaped_like_an_html_anchor_fragment() {
    for tags in RULE_TAGS {
        assert!(
            !tags.doc_slug.is_empty()
                && tags
                    .doc_slug
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                && !tags.doc_slug.starts_with('-')
                && !tags.doc_slug.ends_with('-'),
            "{:?} has an invalid doc_slug {:?}",
            tags.rule,
            tags.doc_slug
        );
    }
}

#[test]
fn rule_tags_has_no_duplicate_doc_slugs() {
    let mut seen = HashSet::new();
    for tags in RULE_TAGS {
        assert!(
            seen.insert(tags.doc_slug),
            "{:?} shares its doc_slug {:?} with another rule",
            tags.rule,
            tags.doc_slug
        );
    }
}

#[test]
fn doc_url_links_to_the_website_lint_anchor() {
    let tags = tags_for(crate::trailing_whitespace::RULE).unwrap();
    assert_eq!(
        tags.doc_url(),
        "https://papyrus-lint.idrinth.de/#lint-trailing-whitespace"
    );
}
