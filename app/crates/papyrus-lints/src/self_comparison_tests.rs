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

#[test]
fn flags_equality_of_a_local_to_itself() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count == count\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("always true"));
}

#[test]
fn flags_inequality_of_a_local_to_itself() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Actor actor)\n    If actor != actor\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("always false"));
}

#[test]
fn flags_lte_and_gte_of_a_local_to_itself() {
    let lte = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count <= count\n    EndIf\nEndFunction\n",
    );
    let gte = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count >= count\n    EndIf\nEndFunction\n",
    );

    assert_eq!(lte.len(), 1);
    assert!(lte[0].message.contains("always true"));
    assert_eq!(gte.len(), 1);
    assert!(gte[0].message.contains("always true"));
}

#[test]
fn matches_the_name_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count == Count\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_self_qualified_property_compared_to_itself() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Count Auto\n\nFunction Test()\n    If Self.Count == Self.Count\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_matching_member_chain_compared_to_itself() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(SomeQuest akQuest)\n    If akQuest.Stage == akQuest.Stage\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_bare_name_compared_to_a_self_qualified_one() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Count Auto\n\nFunction Test()\n    If Count == Self.Count\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_strict_relational_ops() {
    let less = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count < count\n    EndIf\nEndFunction\n",
    );
    let greater = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count > count\n    EndIf\nEndFunction\n",
    );

    assert!(less.is_empty());
    assert!(greater.is_empty());
}

#[test]
fn does_not_flag_comparison_of_different_variables() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    Int b = 2\n    If a == b\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_even_when_written_identically_on_both_sides() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(SomeQuest akQuest)\n    If akQuest.GetStage() == akQuest.GetStage()\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_index_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    If a[0] == a[0]\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_comparison_inside_while_and_nested_logic() {
    let while_diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    While count == count\n        count = 0\n    EndWhile\nEndFunction\n",
    );
    let nested = check(
        "ScriptName Example\n\nFunction Test(Bool flag)\n    Int count = 1\n    If flag && count == count\n    EndIf\nEndFunction\n",
    );

    assert_eq!(while_diagnostics.len(), 1);
    assert_eq!(while_diagnostics[0].line, 5);
    assert_eq!(nested.len(), 1);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
        "ScriptName Example\n\nState Waiting\n    Function Test()\n        Int count = 1\n        If count == count\n        EndIf\n    EndFunction\nEndState\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn returns_no_diagnostics_for_a_script_that_fails_to_parse() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\n    If count == count\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn respects_config_off_switch() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Int count = 1\n    If count == count\n    EndIf\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.self_comparison = false;
    let diagnostics = crate::lint(source, &config);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
