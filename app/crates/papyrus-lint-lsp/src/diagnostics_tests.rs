use super::{line_at, to_lsp};
use papyrus_lints::Diagnostic;

#[test]
fn maps_a_warning_to_a_one_character_range() {
    let diagnostic = Diagnostic {
        line: 2,
        column: 4,
        message: "[warning] Line contains trailing whitespace".to_string(),
        rule: "trailing-whitespace",
    };
    let lsp = to_lsp(&diagnostic, "abc ");
    assert_eq!(lsp["severity"], 2);
    assert_eq!(lsp["message"], "Line contains trailing whitespace");
    assert_eq!(lsp["code"], "trailing-whitespace");
    assert_eq!(lsp["source"], "papyrus-lint");
    assert_eq!(lsp["range"]["start"]["line"], 1);
    assert_eq!(lsp["range"]["start"]["character"], 3);
    assert_eq!(lsp["range"]["end"]["character"], 4);
    assert!(lsp["codeDescription"]["href"]
        .as_str()
        .unwrap()
        .contains("trailing-whitespace"));
}

#[test]
fn counts_utf16_code_units() {
    let diagnostic = Diagnostic {
        line: 1,
        column: 2,
        message: "[error] bad".to_string(),
        rule: "not-a-real-rule",
    };
    let lsp = to_lsp(&diagnostic, "😀x");
    assert_eq!(lsp["range"]["start"]["character"], 2);
    assert_eq!(lsp["range"]["end"]["character"], 3);
    assert!(lsp.get("codeDescription").is_none());
}

#[test]
fn maps_info_and_unprefixed_error_messages() {
    let mut diagnostic = Diagnostic {
        line: 0,
        column: 0,
        message: "[info] useful context".to_string(),
        rule: "not-a-real-rule",
    };
    let info = to_lsp(&diagnostic, "");
    assert_eq!(info["severity"], 3);
    assert_eq!(info["message"], "useful context");
    assert_eq!(info["range"]["start"]["line"], 0);

    diagnostic.message = "plain error".to_string();
    let error = to_lsp(&diagnostic, "");
    assert_eq!(error["severity"], 1);
    assert_eq!(error["message"], "plain error");
    assert_eq!(error["range"]["start"]["character"], 0);
    assert_eq!(error["range"]["end"]["character"], 0);
}

#[test]
fn line_lookup_is_one_based_and_safe() {
    assert_eq!(line_at("one\ntwo", 2), "two");
    assert_eq!(line_at("one", 0), "one");
    assert_eq!(line_at("one", 3), "");
}
