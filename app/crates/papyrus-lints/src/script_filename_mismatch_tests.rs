use std::path::Path;

use super::*;

fn tokens(source: &str) -> Vec<papyrus_parser::token::Token> {
    papyrus_parser::tokenize(source).expect("fixture should lex")
}

fn flagged(file_stem: &str, source: &str) -> Diagnostic {
    check(Path::new(file_stem), &tokens(source)).expect("mismatched script name should be flagged")
}

fn check_path(path: &str, source: &str) -> Option<Diagnostic> {
    check(Path::new(path), &tokens(source))
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
    assert!(check_path("Example.psc", "ScriptName Example\n").is_none());
}

#[test]
fn matches_case_insensitively() {
    assert!(check_path("example.psc", "ScriptName EXAMPLE\n").is_none());
    assert!(check_path("Example.psc", "ScriptName example\n").is_none());
}

#[test]
fn ignores_a_script_with_no_scriptname_statement() {
    assert!(check_path("Example.psc", "; comment only\n").is_none());
}

#[test]
fn ignores_a_scriptname_without_a_declared_name() {
    assert!(check_path("Example.psc", "ScriptName").is_none());
}

#[test]
fn ignores_a_scriptname_followed_by_a_non_identifier() {
    assert!(check_path("Example.psc", "ScriptName 123\n").is_none());
}

#[test]
fn ignores_an_incomplete_namespace() {
    assert!(check_path("Example.psc", "ScriptName User:\n").is_none());
}

#[test]
fn ignores_a_namespace_segment_that_is_not_an_identifier() {
    assert!(check_path("Example.psc", "ScriptName User:123\n").is_none());
}

#[test]
fn ignores_an_empty_file_stem() {
    assert!(check_path("", "ScriptName Example\n").is_none());
}

#[test]
fn compares_the_stem_it_is_given_including_dots() {
    // Callers pass `Path::file_stem`, which only strips the last extension.
    // `Quest.v2.psc` therefore arrives as `Quest.v2`. An identifier can't
    // contain a dot, so no `ScriptName` matches that stem.
    let diagnostic = flagged("Quest.v2.psc", "ScriptName Quest\n");

    assert!(diagnostic.message.contains("'Quest.v2.psc'"));
}

#[test]
fn accepts_a_namespaced_script_in_its_matching_relative_path() {
    assert!(check_path("User/MyScript.psc", "ScriptName User:MyScript\n").is_none());
}

#[test]
fn flags_a_namespaced_script_in_a_different_namespace() {
    let diagnostic = check_path("Other/MyScript.psc", "ScriptName User:MyScript\n")
        .expect("wrong namespace should be flagged");

    assert!(diagnostic.message.contains("'User:MyScript'"));
    assert!(diagnostic.message.contains("Other/MyScript.psc"));
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
    assert!(check_path(
        "Other.psc",
        "ScriptName Example ; @disable script-filename-mismatch\n"
    )
    .is_some());
    assert!(check_path("Other.psc", "ScriptName Example ; @disable\n").is_some());
    assert!(check_path(
        "Other.psc",
        "ScriptName Example\n; @disable-file script-filename-mismatch\n"
    )
    .is_some());
}
