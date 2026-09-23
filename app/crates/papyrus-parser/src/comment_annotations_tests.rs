use super::*;

#[test]
fn parses_multiple_annotations_and_separates_arguments() {
    let annotations = parse_line_annotations(
        "Function Old() ; @deprecated Use New() @nodiscard, @disable old-api, style",
    );
    assert_eq!(annotations.len(), 3);
    assert_eq!(annotations[0].name, "deprecated");
    assert_eq!(annotations[0].arguments, "Use New()");
    assert_eq!(annotations[1].name, "nodiscard");
    assert_eq!(annotations[1].arguments, "");
    assert_eq!(annotations[2].name, "disable");
    assert_eq!(annotations[2].arguments, "old-api, style");
}

#[test]
fn ignores_non_line_comment_annotations() {
    assert!(parse_line_annotations(r#"Debug.Trace("; @private")"#).is_empty());
    assert!(parse_line_annotations(";/ @private /;").is_empty());
    assert!(parse_line_annotations("; mail user@example.com").is_empty());
}
