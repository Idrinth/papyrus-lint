use super::*;

fn tokens(source: &str) -> Vec<papyrus_parser::token::Token> {
    papyrus_parser::tokenize(source).expect("fixture should lex")
}

fn flagged(file_stem: &str, source: &str) -> Diagnostic {
    check(file_stem, &tokens(source)).expect("mismatched script name should be flagged")
}

#[test]
fn flags_a_script_name_that_does_not_match_its_file_stem() {
    let diagnostic = flagged("Other", "ScriptName Example\n");

    assert_eq!(diagnostic.line, 1);
    assert_eq!(diagnostic.rule, RULE);
    assert!(diagnostic.message.starts_with("[error]"));
    assert!(diagnostic.message.contains("'Example'"));
    assert!(diagnostic.message.contains("'Other'"));
}

#[test]
fn does_not_flag_a_matching_script_name() {
    assert!(check("Example", &tokens("ScriptName Example\n")).is_none());
}

#[test]
fn matches_case_insensitively() {
    assert!(check("example", &tokens("ScriptName EXAMPLE\n")).is_none());
    assert!(check("Example", &tokens("ScriptName example\n")).is_none());
}

#[test]
fn ignores_a_script_with_no_scriptname_statement() {
    assert!(check("Example", &tokens("; comment only\n")).is_none());
}

#[test]
fn ignores_a_scriptname_without_a_declared_name() {
    assert!(check("Example", &tokens("ScriptName")).is_none());
}

#[test]
fn ignores_a_scriptname_followed_by_a_non_identifier() {
    assert!(check("Example", &tokens("ScriptName 123\n")).is_none());
}

#[test]
fn ignores_an_incomplete_namespace() {
    assert!(check("Example", &tokens("ScriptName User:\n")).is_none());
}

#[test]
fn ignores_a_namespace_segment_that_is_not_an_identifier() {
    assert!(check("Example", &tokens("ScriptName User:123\n")).is_none());
}

#[test]
fn ignores_an_empty_file_stem() {
    assert!(check("", &tokens("ScriptName Example\n")).is_none());
}

#[test]
fn compares_the_stem_it_is_given_including_dots() {
    // Callers pass `Path::file_stem`, which only strips the last extension.
    // `Quest.v2.psc` therefore arrives as `Quest.v2`. An identifier can't
    // contain a dot, so no `ScriptName` matches that stem.
    let diagnostic = flagged("Quest.v2", "ScriptName Quest\n");

    assert!(diagnostic.message.contains("'Quest.v2'"));
}

#[test]
fn does_not_flag_a_namespaced_script_name_matching_only_its_final_segment() {
    assert!(check("MyScript", &tokens("ScriptName User:MyScript\n")).is_none());
}

#[test]
fn flags_a_namespaced_script_name_whose_final_segment_does_not_match() {
    let diagnostic = flagged("Other", "ScriptName User:MyScript\n");

    assert!(diagnostic.message.contains("'User:MyScript'"));
    assert!(diagnostic.message.contains("'Other'"));
}

#[test]
fn reports_the_scriptname_identifiers_own_position() {
    let diagnostic = flagged("Other", "\n\nScriptName   Example\n");

    assert_eq!(diagnostic.line, 3);
    assert_eq!(diagnostic.column, 14);
}

#[test]
fn does_not_honor_a_disable_comment_itself_since_the_caller_filters_it_in() {
    // Callers merge this into
    // `lint_with_external_arguments_and_extra_diagnostics`, which filters
    // the directive (and counts it as used). That merge is covered by the
    // CLI and desktop tests, not here.
    assert!(check(
        "Other",
        &tokens("ScriptName Example ; @disable script-filename-mismatch\n")
    )
    .is_some());
    assert!(check("Other", &tokens("ScriptName Example ; @disable\n")).is_some());
    assert!(check(
        "Other",
        &tokens("ScriptName Example\n; @disable-file script-filename-mismatch\n")
    )
    .is_some());
}
