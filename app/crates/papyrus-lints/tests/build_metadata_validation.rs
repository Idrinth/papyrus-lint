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

use metadata::{validate, RuleMetadata};

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
