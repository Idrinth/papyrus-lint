use super::*;

#[test]
fn extra_rule_ids_are_all_known() {
    for id in EXTRA_RULE_IDS {
        assert!(
            KNOWN_RULE_IDS.contains(id),
            "{id:?} is in EXTRA_RULE_IDS but missing from docs/rules.json"
        );
    }
}
