use super::*;

#[test]
fn disables_a_specific_rule_on_its_line() {
    let disables = Disables::scan("action = 1 ; @disable float-to-int\nother = 2\n");
    assert!(disables.is_disabled(1, "float-to-int"));
    assert!(!disables.is_disabled(1, "strict-boolean"));
    assert!(!disables.is_disabled(2, "float-to-int"));
}

#[test]
fn disables_are_case_insensitive() {
    let disables = Disables::scan("action = 1 ; @DISABLE Float-To-Int\n");
    assert!(disables.is_disabled(1, "float-to-int"));
}

#[test]
fn disables_multiple_comma_separated_rules() {
    let disables = Disables::scan("Foo(1,2) ; @disable comma-spacing, float-to-int\n");
    assert!(disables.is_disabled(1, "comma-spacing"));
    assert!(disables.is_disabled(1, "float-to-int"));
    assert!(!disables.is_disabled(1, "semicolon"));
}

#[test]
fn accepts_mixed_comma_and_whitespace_separators() {
    let disables = Disables::scan("Foo(1,2) ; @disable comma-spacing  float-to-int,semicolon\n");

    assert!(disables.is_disabled(1, "comma-spacing"));
    assert!(disables.is_disabled(1, "float-to-int"));
    assert!(disables.is_disabled(1, "semicolon"));
}

#[test]
fn duplicate_rule_ids_are_recorded_only_once_at_the_first_column() {
    let disables = Disables::scan("value = 1 ; @disable alpha, beta, ALPHA\n");
    let (_, disable) = disables.iter().next().expect("directive should be found");
    let Directive::Rules(rules) = disable else {
        panic!("named rules should not become an all-rules directive");
    };

    assert_eq!(rules.len(), 2);
    assert_eq!((rules[0].id.as_str(), rules[0].column), ("alpha", 22));
    assert_eq!((rules[1].id.as_str(), rules[1].column), ("beta", 29));
}

#[test]
fn bare_disable_suppresses_every_rule_on_the_line() {
    let disables = Disables::scan("action = 1  ; @disable\n");
    assert!(disables.is_disabled(1, "trailing-whitespace"));
    assert!(disables.is_disabled(1, "float-to-int"));
}

#[test]
fn ignores_semicolons_inside_string_literals() {
    let disables = Disables::scan("Debug.Trace(\"a;b @disable float-to-int\")\n");
    assert!(!disables.is_disabled(1, "float-to-int"));
}

#[test]
fn escaped_quotes_do_not_end_a_string_or_expose_its_semicolon() {
    let disables = Disables::scan(r#"Debug.Trace("escaped \"; @disable float-to-int\"")"#);

    assert!(!disables.is_disabled(1, "float-to-int"));
}

#[test]
fn finds_a_directive_after_a_string_containing_a_semicolon() {
    let disables = Disables::scan(r#"Debug.Trace("still; a string") ; @disable semicolon"#);

    assert!(disables.is_disabled(1, "semicolon"));
}

#[test]
fn reports_character_columns_for_unicode_before_a_directive() {
    let disables = Disables::scan("String text = \"λ\" ; @disable comma-spacing\n");
    let (_, disable) = disables.iter().next().expect("directive should be found");
    let Directive::Rules(rules) = disable else {
        panic!("named rules should not become an all-rules directive");
    };

    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].column, 30);
}

#[test]
fn ignores_disable_like_text_without_the_directive_word() {
    let disables = Disables::scan("action = 1 ; @disabled-thing float-to-int\n");
    assert!(!disables.is_disabled(1, "float-to-int"));
}

#[test]
fn ignores_block_comment_openers() {
    let disables = Disables::scan("action = 1 ;/ @disable float-to-int /;\n");
    assert!(!disables.is_disabled(1, "float-to-int"));
}

#[test]
fn lines_without_a_directive_are_not_disabled() {
    let disables = Disables::scan("action = 1\n; just a comment\n");
    assert!(!disables.is_disabled(1, "float-to-int"));
    assert!(!disables.is_disabled(2, "float-to-int"));
}

#[test]
fn disable_file_suppresses_a_specific_rule_on_every_line() {
    let disables = Disables::scan("; @disable-file float-to-int\naction = 1\nother = 2\n");
    assert!(disables.is_disabled(2, "float-to-int"));
    assert!(disables.is_disabled(3, "float-to-int"));
    assert!(!disables.is_disabled(2, "strict-boolean"));
}

#[test]
fn bare_disable_file_suppresses_every_rule_on_every_line() {
    let disables = Disables::scan("; @disable-file\naction = 1\nother = 2\n");
    assert!(disables.is_disabled(2, "trailing-whitespace"));
    assert!(disables.is_disabled(3, "float-to-int"));
}

#[test]
fn disable_file_need_not_be_on_the_first_line() {
    let disables = Disables::scan("action = 1\nother = 2 ; @disable-file comma-spacing\n");
    assert!(disables.is_disabled(1, "comma-spacing"));
    assert!(disables.is_disabled(2, "comma-spacing"));
}

#[test]
fn disable_file_is_case_insensitive_and_supports_multiple_rules() {
    let disables = Disables::scan("; @DISABLE-FILE Comma-Spacing, Float-To-Int\naction = 1\n");
    assert!(disables.is_disabled(2, "comma-spacing"));
    assert!(disables.is_disabled(2, "float-to-int"));
}

#[test]
fn disable_file_does_not_apply_to_a_plain_disable_line() {
    let disables = Disables::scan("action = 1 ; @disable float-to-int\nother = 2\n");
    assert!(!disables.is_disabled(2, "float-to-int"));
}

#[test]
fn ignores_disable_file_like_text_without_the_directive_word() {
    let disables = Disables::scan("action = 1 ; @disable-filetype float-to-int\nother = 2\n");
    assert!(!disables.is_disabled(1, "float-to-int"));
    assert!(!disables.is_disabled(2, "float-to-int"));
}

#[test]
fn disable_file_directive_does_not_also_register_as_a_line_disable() {
    let disables = Disables::scan("; @disable-file float-to-int\n");
    let (_, disable) = disables
        .file_iter()
        .next()
        .expect("file directive should be found");
    let Directive::Rules(rules) = disable else {
        panic!("named rules should not become an all-rules directive");
    };
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].id, "float-to-int");
    assert!(disables.iter().next().is_none());
}

fn rules(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|id| id.to_string()).collect()
}

#[test]
fn add_disable_directive_appends_a_new_comment_to_a_bare_line() {
    let updated = add_disable_directive("action = 1\nother = 2\n", 1, &rules(&["float-to-int"]));
    assert_eq!(updated, "action = 1 ; @disable float-to-int\nother = 2\n");
}

#[test]
fn add_disable_directive_joins_multiple_rules_with_commas() {
    let updated = add_disable_directive(
        "action = 1\n",
        1,
        &rules(&["float-to-int", "strict-boolean"]),
    );
    assert_eq!(
        updated,
        "action = 1 ; @disable float-to-int, strict-boolean\n"
    );
}

#[test]
fn add_disable_directive_extends_an_unrelated_trailing_comment() {
    let updated = add_disable_directive("action = 1 ; some note\n", 1, &rules(&["float-to-int"]));
    assert_eq!(updated, "action = 1 ; some note @disable float-to-int\n");
}

#[test]
fn add_disable_directive_merges_into_an_existing_rule_list() {
    let updated = add_disable_directive(
        "action = 1 ; @disable float-to-int\n",
        1,
        &rules(&["strict-boolean"]),
    );
    assert_eq!(
        updated,
        "action = 1 ; @disable float-to-int, strict-boolean\n"
    );
}

#[test]
fn add_disable_directive_skips_rules_already_covered() {
    let updated = add_disable_directive(
        "action = 1 ; @disable float-to-int, strict-boolean\n",
        1,
        &rules(&["strict-boolean", "float-to-int"]),
    );
    assert_eq!(
        updated,
        "action = 1 ; @disable float-to-int, strict-boolean\n"
    );
}

#[test]
fn add_disable_directive_leaves_a_bare_disable_untouched() {
    let updated = add_disable_directive("action = 1 ; @disable\n", 1, &rules(&["float-to-int"]));
    assert_eq!(updated, "action = 1 ; @disable\n");
}

#[test]
fn add_disable_directive_only_touches_the_target_line() {
    let updated = add_disable_directive("action = 1\nother = 2\n", 2, &rules(&["float-to-int"]));
    assert_eq!(updated, "action = 1\nother = 2 ; @disable float-to-int\n");
}

#[test]
fn add_disable_directive_preserves_a_trailing_carriage_return() {
    let updated = add_disable_directive("action = 1\r\n", 1, &rules(&["float-to-int"]));
    assert_eq!(updated, "action = 1 ; @disable float-to-int\r\n");
}

#[test]
fn add_disable_directive_is_a_noop_for_an_empty_rule_list() {
    let source = "action = 1\n";
    assert_eq!(add_disable_directive(source, 1, &[]), source);
}

#[test]
fn add_disable_directive_is_a_noop_for_an_out_of_range_line() {
    let source = "action = 1\n";
    assert_eq!(
        add_disable_directive(source, 99, &rules(&["float-to-int"])),
        source
    );
    assert_eq!(
        add_disable_directive(source, 0, &rules(&["float-to-int"])),
        source
    );
}
