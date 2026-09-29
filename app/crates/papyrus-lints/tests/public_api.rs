//! Black-box tests for the crate-level lint and repair entry points.

use papyrus_lints::{
    lint, lint_with_external_arguments, repair, repair_filtered, repair_filtered_by_tag,
    restrict_to_line, tags::tags_for, Config, Diagnostic, ExternalSignatures, NamedArguments,
    ParamInfo, FIXABLE_RULE_IDS, KNOWN_RULE_IDS,
};
use papyrus_parser::ast::TypeName;
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
