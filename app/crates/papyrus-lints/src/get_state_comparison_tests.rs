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

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}

#[test]
fn flags_a_comparison_against_an_undeclared_state() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("'Missing'"));
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn flags_a_not_equal_comparison_too() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetState() != \"Missing\"\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_the_literal_on_the_left_hand_side_too() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If \"Missing\" == GetState()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_comparison_against_a_state_declared_locally() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    If GetState() == \"Active\"\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_the_state_name_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    If GetState() == \"active\"\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_comparison_against_the_empty_state() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetState() == \"\"\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_self_qualified_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If self.GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_call_through_another_object() {
    // GetState always acts on `self`; a call qualified by something
    // else is a different function entirely (or invalid), not this
    // lint's concern.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Example akOther)\n    If akOther.GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn skips_a_non_literal_comparison() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String stateName)\n    If GetState() == stateName\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unrelated_equality_comparison() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_undeclared_target_without_a_resolver_when_extending_another_script() {
    // The target might be declared on a script further up `Extends`
    // that this crate can't resolve on its own; see `check_with`.
    let diagnostics = check(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_comparisons_in_nested_control_flow() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Test(Bool condition)
    If condition
        If GetState() == "Missing"
        EndIf
    EndIf
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_comparisons_inside_state_functions_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        If GetState() == \"Missing\"\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

struct FakeExternalWithAncestorState;

impl ExternalSignatures for FakeExternalWithAncestorState {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn has_state(&mut self, type_name: &str, state_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("BaseScript")
            && state_name.eq_ignore_ascii_case("FromParent")
    }
}

#[test]
fn does_not_flag_a_target_resolved_through_the_extends_ancestry() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    If GetState() == \"FromParent\"\n    EndIf\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_target_not_found_anywhere_in_the_extends_ancestry() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    If GetState() == \"StillMissing\"\n    EndIf\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'StillMissing'"));
}

#[test]
fn flags_a_comparison_nested_in_a_call_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    SomeCall(flag = GetState() == \"Missing\")\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Missing'"));
}

#[test]
fn walks_an_indexed_argument_without_crashing() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String[] names, Int i)\n    Debug.Trace(names[i])\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn line_and_file_disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\" ; @disable get-state-comparison\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file get-state-comparison\nScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source = "ScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.get_state_comparison = false;

    let diagnostics = crate::lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn check_with_returns_no_diagnostics_without_an_ast() {
    let diagnostics = super::check_with(None, &mut FakeExternalWithAncestorState);

    assert!(diagnostics.is_empty());
}

#[test]
fn check_with_walks_every_statement_location() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test(Bool condition)
    Bool initialized = GetState() == "Initializer"
    initialized = GetState() == "Assignment"
    GetState() == "Expression"
    If GetState() == "IfCondition"
        GetState() == "IfBody"
    ElseIf condition
        GetState() == "ElseIfBody"
    Else
        GetState() == "ElseBody"
    EndIf
    While GetState() == "WhileCondition"
        GetState() == "WhileBody"
    EndWhile
    Return GetState() == "Return"
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 10);
}

#[test]
fn check_with_walks_nested_binary_expressions_and_visits_assignment_targets() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test(Bool[] values)
    values[GetState() == "Target"] = (GetState() == "Nested") == false
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("'Target'"));
    assert!(diagnostics[1].message.contains("'Nested'"));
}

#[test]
fn skips_get_state_calls_with_arguments_and_non_equality_operators() {
    let diagnostics = check(
        r#"ScriptName Example

Function Test()
    If GetState("argument") == "Missing"
    EndIf
    If GetState() > "Missing"
    EndIf
EndFunction
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn get_state_name_and_self_qualifier_are_case_insensitive() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    If SELF.gEtStAtE() == \"Missing\"\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn fake_external_function_lookup_returns_none() {
    assert!(FakeExternalWithAncestorState
        .lookup("BaseScript", "SomeFunction")
        .is_none());
}
