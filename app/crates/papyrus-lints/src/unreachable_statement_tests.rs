use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
    )
}

#[test]
fn flags_statement_after_return_in_function_body() {
    let source = "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn flags_every_statement_after_the_first_return() {
    let source = "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\n    Int j = 2\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[1].line, 6);
}

#[test]
fn does_not_flag_a_trailing_return() {
    let source = "ScriptName Example\n\nFunction Test()\n    Int i = 1\n    Return\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_function_with_no_return() {
    let source = "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn flags_statement_after_return_inside_if_branch() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Return\n        Int i = 1\n    EndIf\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_statement_after_return_inside_else_branch() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Int i = 1\n    Else\n        Return\n        Int j = 2\n    EndIf\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn flags_statement_after_return_inside_else_if_branch() {
    let source = "ScriptName Example\n\nFunction Test()\n    If false\n        Int i = 1\n    ElseIf true\n        Return\n        Int j = 2\n    EndIf\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn flags_statement_after_return_inside_while_body() {
    let source = "ScriptName Example\n\nFunction Test()\n    While true\n        Return\n        Int i = 1\n    EndWhile\nEndFunction\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn a_return_inside_an_if_does_not_flag_statements_after_the_if_itself() {
    let source = "ScriptName Example\n\nFunction Test()\n    If true\n        Return\n    EndIf\n    Int i = 1\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn reports_the_line_of_each_unreachable_statement_kind() {
    let source = "ScriptName Example\n\nFunction Test()\n    Int i = 0\n    Return\n    Return\n    i = 1\n    Test()\n    If true\n    EndIf\n    While true\n    EndWhile\nEndFunction\n";

    let diagnostics = check(source);
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.line)
        .collect();

    assert_eq!(lines, [6, 7, 8, 9, 11]);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.column == 1));
}

#[test]
fn still_checks_the_body_of_an_unreachable_compound_statement() {
    let source = "ScriptName Example\n\nFunction Test()\n    Return\n    If true\n        Return\n        Int i = 1\n    EndIf\nEndFunction\n";

    let diagnostics = check(source);
    let lines: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.line)
        .collect();

    assert_eq!(lines, [5, 7]);
}

#[test]
fn checks_event_bodies() {
    let source = "ScriptName Example\n\nEvent OnInit()\n    Return\n    Int i = 1\nEndEvent\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let source = "ScriptName Example\n\nState Active\n    Function Test()\n        Return\n        Int i = 1\n    EndFunction\nEndState\n";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
