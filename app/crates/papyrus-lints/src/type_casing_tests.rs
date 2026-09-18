use super::*;

fn check(source: &str, style: Style) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        type_casing: style,
        ..Default::default()
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::argument_types::NoExternalSignatures,
    )
}

fn repair(source: &str, style: Style) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        type_casing: style,
        ..Default::default()
    };
    super::repair(source, ast.as_ref(), tokens.as_deref(), &config)
}

#[test]
fn pascal_case_accepts_a_conforming_name() {
    assert!(check("ScriptName MyQuestScript\n", Style::PascalCase).is_empty());
}

#[test]
fn pascal_case_flags_a_lowercase_start() {
    let diagnostics = check("ScriptName myQuestScript\n", Style::PascalCase);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("myQuestScript"));
    assert!(diagnostics[0].message.contains("PascalCase"));
    // A letter-casing-only rewrite ("MyQuestScript") fixes this one, so
    // it must not be flagged as unfixable.
    assert!(!diagnostics[0].message.contains("no automatic fix"));
}

#[test]
fn pascal_case_flags_underscores() {
    let diagnostics = check("ScriptName My_QuestScript\n", Style::PascalCase);
    assert_eq!(diagnostics.len(), 1);
    // Removing the underscore would be a substantive rename `repair`
    // won't make, so the diagnostic must say so rather than implying an
    // automatic fix is available.
    assert!(diagnostics[0].message.contains("no automatic fix"));
}

#[test]
fn ignores_a_creationkit_fragment_style_prefix() {
    // CreationKit itself names dialogue Topic Info fragment scripts this
    // way (`<QuestEditorID>__TIF__<FormID>`); the two leading
    // uppercase-acronym-plus-underscores segments are stripped before
    // checking casing, so the remainder (all digits/uppercase) matches
    // PascalCase trivially instead of being flagged as an unfixable
    // underscore violation.
    assert!(check("ScriptName IDR__TIF__050000F5\n", Style::PascalCase).is_empty());
}

#[test]
fn ignores_a_mod_acronym_prefix() {
    // A modder's own uppercase-acronym prefix (single underscore) is
    // stripped the same way, so only `MyQuestScript` is checked.
    assert!(check("ScriptName USSEP_MyQuestScript\n", Style::PascalCase).is_empty());
}

#[test]
fn does_not_treat_a_single_leading_capital_as_a_prefix() {
    // "My_QuestScript" isn't a recognized acronym prefix (the uppercase
    // run before the underscore is just the name's own first letter),
    // so this is still flagged, and still unfixable since removing the
    // underscore would be a substantive rename.
    let diagnostics = check("ScriptName My_QuestScript\n", Style::PascalCase);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("no automatic fix"));
}

#[test]
fn repair_leaves_a_recognized_prefix_untouched() {
    assert_eq!(
        repair("ScriptName USSEP_myQuestScript\n", Style::PascalCase),
        "ScriptName USSEP_MyQuestScript\n"
    );
}

#[test]
fn strip_known_prefix_handles_up_to_two_segments() {
    assert_eq!(strip_known_prefix("IDR__TIF__050000F5"), "050000F5");
    assert_eq!(strip_known_prefix("USSEP_MyQuestScript"), "MyQuestScript");
    assert_eq!(strip_known_prefix("MyQuestScript"), "MyQuestScript");
    assert_eq!(strip_known_prefix("My_QuestScript"), "My_QuestScript");
    // A name that's entirely a prefix (nothing left after stripping) is
    // returned unchanged rather than reduced to an empty string.
    assert_eq!(strip_known_prefix("USSEP_"), "USSEP_");
}

#[test]
fn camel_case_accepts_a_conforming_name() {
    assert!(check("ScriptName myQuestScript\n", Style::CamelCase).is_empty());
}

#[test]
fn camel_case_flags_an_uppercase_start() {
    assert_eq!(
        check("ScriptName MyQuestScript\n", Style::CamelCase).len(),
        1
    );
}

#[test]
fn lowercase_accepts_an_all_lowercase_name() {
    assert!(check("ScriptName myquestscript\n", Style::Lowercase).is_empty());
}

#[test]
fn lowercase_flags_any_uppercase_letter() {
    assert_eq!(
        check("ScriptName myQuestscript\n", Style::Lowercase).len(),
        1
    );
}

#[test]
fn uppercase_accepts_an_all_uppercase_name() {
    assert!(check("ScriptName MYQUESTSCRIPT\n", Style::Uppercase).is_empty());
}

#[test]
fn uppercase_flags_any_lowercase_letter() {
    assert_eq!(
        check("ScriptName MyQUESTSCRIPT\n", Style::Uppercase).len(),
        1
    );
}

#[test]
fn lowercase_and_uppercase_allow_underscores_and_digits() {
    assert!(check("ScriptName quest_script2\n", Style::Lowercase).is_empty());
    assert!(check("ScriptName QUEST_SCRIPT2\n", Style::Uppercase).is_empty());
}

#[test]
fn names_without_letters_do_not_have_a_case_violation() {
    assert!(check("ScriptName _123\n", Style::Lowercase).is_empty());
    assert!(check("ScriptName _123\n", Style::Uppercase).is_empty());

    // PascalCase and camelCase still reject this identifier because
    // those conventions independently prohibit underscores.
    assert_eq!(check("ScriptName _123\n", Style::PascalCase).len(), 1);
    assert_eq!(check("ScriptName _123\n", Style::CamelCase).len(), 1);
}

#[test]
fn every_style_uses_its_config_value_in_the_diagnostic() {
    for (style, name, label) in [
        (Style::PascalCase, "example", "PascalCase"),
        (Style::CamelCase, "Example", "camelCase"),
        (Style::Lowercase, "ExAmPlE", "lowercase"),
        (Style::Uppercase, "ExAmPlE", "UPPERCASE"),
    ] {
        let diagnostics = check(&format!("ScriptName {name}\n"), style);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains(label));
    }
}

#[test]
fn reports_the_declared_name_position_not_the_keyword() {
    let diagnostics = check(
        "Scriptname   myQuestScript Extends Quest\n",
        Style::PascalCase,
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].column, 14);
}

#[test]
fn script_with_no_scriptname_statement_is_unflagged() {
    assert!(check("Function Foo()\nEndFunction\n", Style::PascalCase).is_empty());
}

#[test]
fn incomplete_or_non_identifier_declarations_are_unflagged() {
    assert!(check("ScriptName", Style::PascalCase).is_empty());
    assert!(check("ScriptName Int\n", Style::PascalCase).is_empty());
}

#[test]
fn a_script_that_fails_to_lex_is_left_unchecked() {
    assert!(check("ScriptName Example \"unterminated\n", Style::PascalCase).is_empty());
}

#[test]
fn extends_target_casing_is_never_checked() {
    // Only the declared name (Example) is checked, not the Extends
    // target (badlyCasedParent) which belongs to another script.
    assert!(check(
        "ScriptName Example Extends badlyCasedParent\n",
        Style::PascalCase
    )
    .is_empty());
}

#[test]
fn repair_applies_each_configured_style() {
    assert_eq!(
        repair("ScriptName myQuestScript\n", Style::PascalCase),
        "ScriptName MyQuestScript\n"
    );
    assert_eq!(
        repair("ScriptName MyQuestScript\n", Style::CamelCase),
        "ScriptName myQuestScript\n"
    );
    assert_eq!(
        repair("ScriptName MyQuestScript\n", Style::Lowercase),
        "ScriptName myquestscript\n"
    );
    assert_eq!(
        repair("ScriptName MyQuestScript\n", Style::Uppercase),
        "ScriptName MYQUESTSCRIPT\n"
    );
}

#[test]
fn repair_changes_only_the_declared_name_and_is_idempotent() {
    let source = "ScriptName myScript Extends badly_cased_parent\r\n";
    let repaired = repair(source, Style::PascalCase);

    assert_eq!(
        repaired,
        "ScriptName MyScript Extends badly_cased_parent\r\n"
    );
    assert_eq!(repair(&repaired, Style::PascalCase), repaired);
}

#[test]
fn repair_finds_the_declaration_after_an_earlier_line() {
    let source = "; generated file\r\nScriptName myScript Extends Parent\r\n";
    assert_eq!(
        repair(source, Style::PascalCase),
        "; generated file\r\nScriptName MyScript Extends Parent\r\n"
    );
}

#[test]
fn lowercase_and_uppercase_repairs_preserve_non_letters() {
    assert_eq!(
        repair("ScriptName Quest_Script2\n", Style::Lowercase),
        "ScriptName quest_script2\n"
    );
    assert_eq!(
        repair("ScriptName Quest_Script2\n", Style::Uppercase),
        "ScriptName QUEST_SCRIPT2\n"
    );
}

#[test]
fn repair_does_not_make_a_substantive_rename() {
    for style in [Style::PascalCase, Style::CamelCase] {
        let source = "ScriptName my_questScript\n";
        assert_eq!(repair(source, style), source);
    }
}

#[test]
fn repair_leaves_invalid_or_declaration_free_source_unchanged() {
    for source in [
        "Function Foo()\nEndFunction\n",
        "ScriptName Example \"unterminated\n",
        "ScriptName",
        "ScriptName Int\n",
    ] {
        assert_eq!(repair(source, Style::PascalCase), source);
    }
}

#[test]
fn pascal_case_is_the_default_style() {
    assert_eq!(Style::default(), Style::PascalCase);
}
