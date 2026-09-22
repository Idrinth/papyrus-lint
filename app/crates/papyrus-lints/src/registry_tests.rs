use super::*;
use crate::config::Rules;

#[test]
fn extra_rule_ids_are_all_known() {
    for id in EXTRA_RULE_IDS {
        assert!(
            KNOWN_RULE_IDS.contains(id),
            "{id:?} is in EXTRA_RULE_IDS but missing from shared/rules.json"
        );
    }
}

#[test]
fn generated_rules_have_one_field_per_known_id() {
    let value = serde_json::to_value(Rules::default()).expect("Rules serializes");
    let object = value.as_object().expect("Rules serializes as an object");
    assert_eq!(
        object.len(),
        KNOWN_RULE_IDS.len(),
        "Rules fields and shared/rules.json must stay 1:1"
    );
}
