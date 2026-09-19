//! Tests for [`crate::Diagnostic`].

use super::super::*;

#[test]
fn diagnostic_level_parses_the_leading_tag() {
    let tagged = |message: &str| Diagnostic {
        line: 1,
        column: 1,
        message: message.to_string(),
        rule: "some-rule",
    };

    assert_eq!(tagged("[error] boom").level(), "error");
    assert_eq!(tagged("[warning] hmm").level(), "warning");
    assert_eq!(tagged("[info] fyi").level(), "info");
    assert_eq!(tagged("No recognized level prefix here").level(), "error");
}
