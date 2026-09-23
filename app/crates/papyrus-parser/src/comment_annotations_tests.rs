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

#[test]
fn reports_annotation_source_locations_with_unicode_and_trailing_separators() {
    let line = "é = 1 ;  @first one,  @second two,   ";
    let annotations = parse_line_annotations(line);

    assert_eq!(annotations.len(), 2);
    assert_eq!(annotations[0].column, 10);
    assert_eq!(
        &line[annotations[0].byte_start..annotations[0].byte_end],
        "@first one"
    );
    assert_eq!(
        &line[annotations[0].arguments_byte_start..annotations[0].byte_end],
        "one"
    );
    assert_eq!(annotations[1].arguments, "two");
    assert_eq!(
        &line[annotations[1].byte_start..annotations[1].byte_end],
        "@second two"
    );
}

#[test]
fn recognizes_only_annotation_boundaries_and_skips_escaped_string_delimiters() {
    assert!(parse_line_annotations(r#"Debug.Trace("escaped \";\" text")"#).is_empty());

    let annotations = parse_line_annotations("value = 1 ; prefix@not_one, @, @_valid-name arg");
    assert_eq!(annotations.len(), 1);
    assert_eq!(annotations[0].name, "_valid-name");
    assert_eq!(annotations[0].arguments, "arg");
}
