#![allow(dead_code)]

#[path = "../build_support/metadata.rs"]
mod metadata;

// `metadata.rs`'s production loader refers to this type; these tests exercise
// validation without running Cargo's build script or touching the filesystem.
pub struct BuildContext;
impl BuildContext {
    pub fn load_json<T>(&self, _: &str, _: &str) -> T {
        unreachable!()
    }
}

use metadata::{config_key, module_name, order_by_config, validate, RuleMetadata};

fn rule(id: &str) -> RuleMetadata {
    RuleMetadata {
        id: id.to_string(),
        name: id.to_string(),
        tags: vec!["style".to_string()],
        importance: "medium".to_string(),
        definition: String::new(),
        fixable: false,
        visitor: "ast".to_string(),
        repair_order: None,
        enabled_by_default: true,
    }
}

#[test]
fn accepts_consistent_metadata() {
    assert_eq!(validate(&[rule("example")]), Ok(()));
}

#[test]
fn rejects_duplicate_ids() {
    let error = validate(&[rule("duplicate"), rule("duplicate")]).unwrap_err();
    assert!(error
        .to_string()
        .contains("lists `duplicate` more than once"));
}

#[test]
fn rejects_invalid_tags_and_importance() {
    let mut no_tags = rule("no-tags");
    no_tags.tags.clear();
    assert!(validate(&[no_tags])
        .unwrap_err()
        .to_string()
        .contains("has no tags"));
    let mut invalid = rule("invalid");
    invalid.importance = "urgent".to_string();
    assert!(validate(&[invalid])
        .unwrap_err()
        .to_string()
        .contains("unknown importance"));
    let mut visitor = rule("visitor");
    visitor.visitor = "cfg".to_string();
    assert!(validate(&[visitor])
        .unwrap_err()
        .to_string()
        .contains("unknown visitor"));
}

#[test]
fn rejects_invalid_repair_metadata() {
    let mut not_fixable = rule("not-fixable");
    not_fixable.repair_order = Some(1);
    assert!(validate(&[not_fixable])
        .unwrap_err()
        .to_string()
        .contains("not fixable"));
    let mut missing_order = rule("missing-order");
    missing_order.fixable = true;
    assert!(validate(&[missing_order])
        .unwrap_err()
        .to_string()
        .contains("needs `repair_order`"));
    let mut gap = rule("gap");
    gap.fixable = true;
    gap.repair_order = Some(2);
    assert!(validate(&[gap])
        .unwrap_err()
        .to_string()
        .contains("without gaps"));
}

#[test]
fn maps_rule_ids_to_rust_identifiers() {
    assert_eq!(module_name("comma-spacing"), "comma_spacing");
    assert_eq!(module_name("float-to-int"), "float_int_conversion");
    assert_eq!(module_name("event-signature-mismatch"), "event_signature");

    assert_eq!(config_key("comma-spacing"), "comma_spacing");
    assert_eq!(config_key("float-to-int"), "float_int_conversion");
    assert_eq!(config_key("too-many-named-states"), "too_many_states");
}

#[test]
fn accepts_external_repairs_without_an_apply_repairs_order() {
    let mut unused_import = rule("unused-import");
    unused_import.fixable = true;

    assert_eq!(validate(&[unused_import]), Ok(()));
}

#[test]
fn rejects_repair_orders_on_special_case_rules() {
    let mut project_rule = rule("stale-compiled-output");
    project_rule.fixable = true;
    project_rule.repair_order = Some(1);
    assert!(validate(&[project_rule])
        .unwrap_err()
        .to_string()
        .contains("project/post-pass rule"));

    let mut external_repair = rule("unused-import");
    external_repair.fixable = true;
    external_repair.repair_order = Some(1);
    assert!(validate(&[external_repair])
        .unwrap_err()
        .to_string()
        .contains("repaired outside apply_repairs"));
}

#[test]
fn orders_rules_to_match_the_configuration() {
    let rules = vec![rule("trailing-whitespace"), rule("float-to-int")];
    let field_order = vec![
        "float_int_conversion".to_string(),
        "trailing_whitespace".to_string(),
    ];

    let ordered = order_by_config(&rules, &field_order).unwrap();

    assert_eq!(
        ordered
            .into_iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>(),
        ["float-to-int", "trailing-whitespace"]
    );
}

#[test]
fn rejects_configuration_keys_without_rule_metadata() {
    let error =
        order_by_config(&[rule("comma-spacing")], &["unknown_rule".to_string()]).unwrap_err();

    assert!(error
        .to_string()
        .contains("lists rules.unknown_rule but shared/rules.json has no matching id"));
}

#[test]
fn rejects_rule_metadata_missing_from_the_configuration() {
    let error = order_by_config(
        &[rule("trailing-whitespace"), rule("comma-spacing")],
        &["comma_spacing".to_string()],
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("missing rules: [\"trailing_whitespace\"]"));
}

#[test]
fn rejects_rule_ids_that_collide_on_the_same_configuration_key() {
    let error = order_by_config(
        &[rule("same-name"), rule("same_name")],
        &["same_name".to_string()],
    )
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("duplicate Rules field `same_name`"));
}
