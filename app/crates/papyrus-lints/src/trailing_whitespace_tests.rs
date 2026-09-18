use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn repair(source: &str) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
    )
}

#[test]
fn flags_trailing_spaces() {
    let diagnostics = check("ScriptName Example  \n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].column, 19);
}

#[test]
fn flags_trailing_tabs() {
    let diagnostics = check("Int x = 1\t\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].column, 10);
}

#[test]
fn ignores_clean_lines() {
    let diagnostics = check("ScriptName Example\n\nInt x = 1\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_trailing_carriage_return() {
    let diagnostics = check("ScriptName Example\r\nInt x = 1\r\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_multiple_lines_independently() {
    let source = "Line one \nLine two\nLine three\t\n";
    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[1].line, 3);
}

#[test]
fn flags_whitespace_only_line() {
    let diagnostics = check("ScriptName Example\n   \nInt x = 1\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 2);
    assert_eq!(diagnostics[0].column, 1);
}

#[test]
fn flags_last_line_without_trailing_newline() {
    let diagnostics = check("ScriptName Example   ");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
}

#[test]
fn reports_columns_in_characters_for_non_ascii_source() {
    let diagnostics = check("String greeting = \"Héllo 🌍\"  \n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].column, 28);
}

#[test]
fn repairs_trailing_spaces() {
    assert_eq!(repair("ScriptName Example  \n"), "ScriptName Example\n");
}

#[test]
fn repairs_trailing_tabs() {
    assert_eq!(repair("Int x = 1\t\n"), "Int x = 1\n");
}

#[test]
fn leaves_clean_lines_untouched() {
    let source = "ScriptName Example\n\nInt x = 1\n";
    assert_eq!(repair(source), source);
}

#[test]
fn empty_source_is_unchanged() {
    assert_eq!(repair(""), "");
    assert!(check("").is_empty());
}

#[test]
fn preserves_crlf_line_endings() {
    assert_eq!(repair("Int x = 1  \r\n"), "Int x = 1\r\n");
}

#[test]
fn preserves_each_ending_in_a_mixed_line_ending_file() {
    assert_eq!(
        repair("Line one  \r\nLine two\t\nLine three   "),
        "Line one\r\nLine two\nLine three"
    );
}

#[test]
fn preserves_leading_whitespace_and_line_without_trailing_newline() {
    assert_eq!(repair("\tInt x = 1   "), "\tInt x = 1");
}

#[test]
fn clears_whitespace_only_lines() {
    assert_eq!(
        repair("ScriptName Example\n   \nInt x = 1\n"),
        "ScriptName Example\n\nInt x = 1\n"
    );
}

#[test]
fn repairs_multiple_lines_independently() {
    let source = "Line one \nLine two\nLine three\t\n";
    assert_eq!(repair(source), "Line one\nLine two\nLine three\n");
}

#[test]
fn fragment_code_wrapper_trailing_whitespace_is_left_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment  \nScriptname Example Extends TopicInfo Hidden\nFunction Fragment_0(ObjectReference akSpeakerRef)\n;BEGIN CODE\nakSpeaker.RemoveItem(x, 1, false, PlayerRef)  \n;END CODE\nEndFunction\t\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";
    assert!(check(source).iter().all(|d| d.line == 5));
    let repaired = repair(source);
    assert!(repaired.starts_with(
        ";BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment  \n"
    ));
    assert!(repaired.contains("EndFunction\t\n"));
    assert!(repaired.contains("akSpeaker.RemoveItem(x, 1, false, PlayerRef)\n"));
}

#[test]
fn repaired_source_has_no_remaining_diagnostics() {
    let source = "Line one \r\nLine two\t\n\tLine three   ";
    let repaired = repair(source);
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_is_idempotent() {
    let source = "Line one \r\nLine two\t\n\tLine three   ";
    let repaired = repair(source);

    assert_eq!(repair(&repaired), repaired);
}
