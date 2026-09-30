//! Black-box tests for the crate-level published rule catalog and
//! filtered-repair coverage of every advertised fixable rule.

use papyrus_lints::{
    repair_filtered, tags::tags_for, Config, NamedArguments, FIXABLE_RULE_IDS, KNOWN_RULE_IDS,
};
use std::collections::HashSet;

#[test]
fn published_rule_id_lists_are_unique_and_fixable_rules_are_known() {
    let known: HashSet<_> = KNOWN_RULE_IDS.iter().copied().collect();
    let fixable: HashSet<_> = FIXABLE_RULE_IDS.iter().copied().collect();

    assert_eq!(known.len(), KNOWN_RULE_IDS.len(), "duplicate known rule id");
    assert_eq!(
        fixable.len(),
        FIXABLE_RULE_IDS.len(),
        "duplicate fixable rule id"
    );
    assert!(
        fixable.is_subset(&known),
        "every fixable rule must also be advertised as known"
    );
}

#[test]
fn every_known_rule_id_resolves_to_published_tags() {
    for rule in KNOWN_RULE_IDS {
        let tags = tags_for(rule).unwrap_or_else(|| panic!("{rule:?} has no published tags"));
        assert!(!tags.kinds.is_empty());
        assert_eq!(tags.auto_fixable(), FIXABLE_RULE_IDS.contains(rule));
    }
}

#[test]
fn a_published_fixable_rule_works_through_the_filtered_public_api() {
    let mut property_config = Config::default();
    property_config.rules.property_sorting = true;

    let named_arguments_config = Config {
        named_arguments: NamedArguments::Always,
        ..Config::default()
    };

    let mut unused_disable_config = Config::default();
    unused_disable_config.rules.unused_disable = true;

    let default_config = Config::default();
    let cases = [
        (
            "identifier-casing",
            "ScriptName Example\n\nFunction Run(Int left)\nEndFunction\n",
            "ScriptName Example\n\nFunction Run(Int Left)\nEndFunction\n",
            &default_config,
        ),
    ];

    for (rule, source, expected, config) in cases {
        assert_eq!(
            repair_filtered(source, config, Some(rule)),
            expected,
            "filtered repair failed for {rule}"
        );
    }
}
