use super::*;

#[test]
fn level_of_reads_the_leading_severity_tag() {
    assert_eq!(level_of("[error] oops"), "error");
    assert_eq!(level_of("[warning] oops"), "warning");
    assert_eq!(level_of("[info] oops"), "info");
}

#[test]
fn level_of_defaults_untagged_messages_to_error() {
    assert_eq!(level_of("no tag here"), "error");
}

#[test]
fn strip_severity_prefix_removes_the_tag_and_following_whitespace() {
    assert_eq!(
        strip_severity_prefix("[warning] trailing whitespace"),
        "trailing whitespace"
    );
    assert_eq!(strip_severity_prefix("[error]no space"), "no space");
}

#[test]
fn strip_severity_prefix_leaves_an_untagged_message_unchanged() {
    assert_eq!(strip_severity_prefix("plain message"), "plain message");
}

#[test]
fn owned_diagnostic_implements_diagnostic_like() {
    let diagnostic = OwnedDiagnostic {
        line: 4,
        column: 2,
        rule: "example-rule".to_string(),
        message: "[info] example".to_string(),
    };

    assert_eq!(diagnostic.line(), 4);
    assert_eq!(diagnostic.column(), 2);
    assert_eq!(diagnostic.rule(), "example-rule");
    assert_eq!(diagnostic.message(), "[info] example");
}

#[test]
fn native_diagnostic_implements_diagnostic_like() {
    let diagnostic = papyrus_lints::Diagnostic {
        line: 1,
        column: 1,
        rule: "example-rule",
        message: "[warning] example".to_string(),
    };

    assert_eq!(diagnostic.line(), 1);
    assert_eq!(diagnostic.column(), 1);
    assert_eq!(diagnostic.rule(), "example-rule");
    assert_eq!(diagnostic.message(), "[warning] example");
}
