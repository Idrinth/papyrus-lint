use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(source, tokens.as_deref())
}

#[test]
fn flags_and_repairs_calls_and_declarations() {
    let source = "Function Add(Int left,Int right)\n  Use(Add(1,2),3)\nEndFunction\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 22));
    assert_eq!(
        repair(source),
        "Function Add(Int left, Int right)\n  Use(Add(1, 2), 3)\nEndFunction\n"
    );
}

#[test]
fn accepts_existing_whitespace_and_multiline_arguments() {
    let source = "Use(1, 2,\t3,\n  4,\r\n  5)\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn ignores_commas_outside_argument_lists_and_inside_strings_or_comments() {
    let source =
        "String value = \"one,two\" ; comment,here\n; / block,comment /;\nInt[] values = [1,2]\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn fragment_code_wrapper_commas_are_left_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef,Actor akSpeaker)
;BEGIN CODE
akSpeaker.RemoveItem(x,1,false,PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = check(source);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(
        repair(source),
        "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(ObjectReference akSpeakerRef,Actor akSpeaker)
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
    );
}

#[test]
fn repair_is_idempotent_and_preserves_unicode() {
    let source = "Show(\"é\",value)\n";
    let repaired = repair(source);
    assert_eq!(repaired, "Show(\"é\", value)\n");
    assert_eq!(repair(&repaired), repaired);
}

#[test]
fn allows_a_trailing_comma_before_a_closing_parenthesis() {
    let source = "Use(value,)\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn an_unmatched_closing_parenthesis_does_not_create_argument_context() {
    let source = ") first,second\nUse(first,second)\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 10));
    assert_eq!(repair(source), ") first,second\nUse(first, second)\n");
}

#[test]
fn invalid_source_is_left_unchanged() {
    let source = "Use(\"unterminated,value)\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn nested_parentheses_keep_argument_context_until_the_outer_call_closes() {
    let source = "Use(Choose(first,second),third)\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.column)
            .collect::<Vec<_>>(),
        vec![17, 25]
    );
    assert_eq!(repair(source), "Use(Choose(first, second), third)\n");
}

#[test]
fn diagnostic_columns_count_unicode_characters_before_the_comma() {
    let diagnostics = check("Show(\"hé\",value)\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 10));
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn comma_at_end_of_input_is_not_flagged() {
    let source = "Use(value,";

    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}
