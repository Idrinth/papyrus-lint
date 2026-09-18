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
fn flags_a_new_array_larger_than_the_engine_maximum() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[200]\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("200"));
    assert!(diagnostics[0].message.contains("128"));
}

#[test]
fn flags_a_new_array_with_a_negative_size() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[-1]\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_a_new_array_at_the_engine_maximum() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[128]\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_new_array_at_zero() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[0]\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_new_array_whose_size_is_not_a_literal() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int n)\n    Int[] a = new Int[n]\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn constant_folding_handles_literal_arithmetic_in_the_size() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[100 + 50]\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn checks_a_new_array_nested_in_a_call_argument() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Consume(new Int[200])\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int[] a = new Int[200]\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn checks_conditions_and_branches_of_an_if() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Int[] a = new Int[200]\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn checks_a_while_loop_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    While flag\n        Int[] a = new Int[200]\n        flag = false\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
