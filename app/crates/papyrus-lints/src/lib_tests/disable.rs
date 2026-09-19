//! Tests for [`crate::add_disable_comment`] and [`crate::is_disabled`].

use super::super::*;

#[test]
fn add_disable_comment_suppresses_the_named_rule_on_the_target_line() {
    let source = "Call(1,2)  \n";
    let config = Config::default();
    assert!(!lint(source, &config).is_empty());

    let updated = add_disable_comment(source, 1, &[comma_spacing::RULE.to_string()]);
    let remaining: Vec<_> = lint(&updated, &config)
        .into_iter()
        .filter(|finding| finding.rule == comma_spacing::RULE)
        .collect();

    assert!(remaining.is_empty());
}

#[test]
fn is_disabled_exposes_line_and_file_directive_checks() {
    let source = concat!(
        "Call(1,2) ; @disable comma-spacing\n",
        "Call(3,4)\n",
        "; @disable-file trailing-whitespace\n",
    );

    assert!(is_disabled(source, 1, comma_spacing::RULE));
    assert!(!is_disabled(source, 2, comma_spacing::RULE));
    assert!(is_disabled(source, 2, trailing_whitespace::RULE));
    assert!(!is_disabled(source, 2, semicolon::RULE));
}
