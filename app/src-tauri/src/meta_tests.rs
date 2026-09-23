use super::*;

#[test]
fn get_app_version_returns_the_crate_version() {
    assert_eq!(get_app_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn list_rule_tags_reports_every_known_rules_metadata() {
    let tags = list_rule_tags();
    assert_eq!(tags.len(), papyrus_lints::tags::RULE_TAGS.len());

    let trailing_whitespace = tags
        .iter()
        .find(|info| info.rule == "trailing-whitespace")
        .expect("trailing-whitespace should be tagged");
    assert!(!trailing_whitespace.description.is_empty());
    assert_eq!(trailing_whitespace.kinds, vec!["style"]);
    assert_eq!(
        trailing_whitespace.importance,
        papyrus_lints::tags::Importance::Low
    );
    assert!(trailing_whitespace.auto_fixable);
    assert_eq!(
        trailing_whitespace.doc_url,
        "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace"
    );

    let argument_types = tags
        .iter()
        .find(|info| info.rule == "argument-types")
        .expect("argument-types should be tagged");
    assert!(!argument_types.auto_fixable);
}
