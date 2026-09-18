use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(source, tokens.as_deref())
}

#[test]
fn ignores_a_single_space() {
    let source = "ScriptName Example\n\nFunction Test()\n    If ! bReady\n    EndIf\nEndFunction\n";
    assert!(check(source).is_empty());
}

#[test]
fn flags_no_space() {
    let diagnostics = check("If !bReady\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn flags_multiple_spaces() {
    let diagnostics = check("If !   bReady\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_tab() {
    let diagnostics = check("If !\tbReady\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn ignores_not_equal_operator() {
    assert!(check("If a != b\nEndIf\n").is_empty());
}

#[test]
fn ignores_exclamation_marks_in_strings_and_comments() {
    let source = "\
String message = \"Look!NoSpace\"
; !comment
{/ !documentation /}
;/ !block
comment /;
";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn malformed_source_is_left_unchanged() {
    // Lints inspect incomplete scripts where possible, but a lexer error
    // must not lead to a partial or incorrectly positioned edit.
    let source = "If !bReady\n    String message = \"unterminated\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn ignores_negation_with_nothing_but_a_newline_after_it() {
    // Inserting a space here would just be trailing whitespace, which
    // the "Trailing whitespace" fix would strip right back off in the
    // combined `repair()` pipeline (it runs after this one), so this
    // lint has nothing useful to say about it.
    assert!(check("If !\nEndIf\n").is_empty());
}

#[test]
fn ignores_negation_with_only_trailing_whitespace_after_it() {
    assert!(check("If !   \nEndIf\n").is_empty());
}

#[test]
fn ignores_negation_with_only_trailing_whitespace_before_crlf() {
    let source = "If !\t  \r\nEndIf\r\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn ignores_negation_at_end_of_file_with_no_trailing_newline() {
    assert!(check("If !").is_empty());
}

#[test]
fn flags_each_negation_independently() {
    let diagnostics = check("If !a || !  b\nEndIf\n");
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn ignores_the_gap_between_chained_negation_operators() {
    // `!!bReady` is two `!` tokens back to back; only the last one in
    // the run needs its own trailing space, so exactly one diagnostic
    // is expected here, for the second `!`.
    let diagnostics = check("If !!bReady\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].column, 5);
}

#[test]
fn ignores_the_gap_in_a_longer_run_of_chained_negation_operators() {
    let diagnostics = check("If !!!bReady\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].column, 6);
}

#[test]
fn ignores_a_chained_negation_run_already_followed_by_a_single_space() {
    assert!(check("If !! bReady\nEndIf\n").is_empty());
}

#[test]
fn fragment_code_wrapper_negations_are_left_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
If !bReady
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = check(source);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn repair_leaves_a_single_space_untouched() {
    let source = "If ! bReady\nEndIf\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_inserts_a_missing_space() {
    assert_eq!(repair("If !bReady\nEndIf\n"), "If ! bReady\nEndIf\n");
}

#[test]
fn repair_collapses_multiple_spaces() {
    assert_eq!(repair("If !   bReady\nEndIf\n"), "If ! bReady\nEndIf\n");
}

#[test]
fn repair_replaces_a_tab_with_a_space() {
    assert_eq!(repair("If !\tbReady\nEndIf\n"), "If ! bReady\nEndIf\n");
}

#[test]
fn repair_leaves_not_equal_operator_alone() {
    let source = "If a != b\nEndIf\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_negation_with_nothing_but_a_newline_after_it_alone() {
    let source = "If !\nEndIf\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_negation_with_only_trailing_whitespace_after_it_alone() {
    let source = "If !   \nEndIf\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_leaves_negation_at_end_of_file_with_no_trailing_newline_alone() {
    let source = "If !";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_fixes_each_negation_independently() {
    assert_eq!(repair("If !a || !  b\nEndIf\n"), "If ! a || ! b\nEndIf\n");
}

#[test]
fn repair_leaves_the_gap_between_chained_negation_operators_alone() {
    assert_eq!(repair("If !!bReady\nEndIf\n"), "If !! bReady\nEndIf\n");
}

#[test]
fn repair_leaves_a_longer_run_of_chained_negation_operators_alone() {
    assert_eq!(repair("If !!!bReady\nEndIf\n"), "If !!! bReady\nEndIf\n");
}

#[test]
fn repair_leaves_an_already_correct_chained_negation_run_untouched() {
    let source = "If !! bReady\nEndIf\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_fixes_the_code_body_but_leaves_the_wrapper_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
If !bReady
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    assert_eq!(
        repair(source),
        "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef)
;BEGIN CODE
If ! bReady
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
    );
}

#[test]
fn repair_result_has_no_remaining_diagnostics() {
    let source = "If !bReady || !  bOther\nEndIf\n";
    let repaired = repair(source);
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_is_idempotent() {
    let repaired = repair("If !   bReady\nEndIf\n");
    assert_eq!(repair(&repaired), repaired);
}
