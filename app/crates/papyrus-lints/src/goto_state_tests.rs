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
fn flags_a_goto_state_call_to_an_undeclared_state() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Missing'"));
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn does_not_flag_a_call_to_a_state_declared_locally() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    GoToState(\"Active\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_the_state_name_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\nEndState\n\nFunction Test()\n    GoToState(\"active\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_switching_to_the_empty_state() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    GoToState(\"\")\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_self_qualified_call() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    self.GoToState(\"Missing\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_call_through_another_object() {
    // GoToState always acts on `self`; a call qualified by something
    // else is a different function entirely (or invalid), not this
    // lint's concern.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Example akOther)\n    akOther.GoToState(\"Missing\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn skips_a_non_literal_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String stateName)\n    GoToState(stateName)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_undeclared_target_without_a_resolver_when_extending_another_script() {
    // The target might be declared on a script further up `Extends`
    // that this crate can't resolve on its own; see `check_with`.
    let diagnostics = check(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_calls_in_nested_control_flow() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Test(Bool condition)
    If condition
        GoToState("Missing")
    EndIf
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_calls_inside_state_functions_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        GoToState(\"Missing\")\n    EndFunction\nEndState\n",
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
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    GoToState(\"FromParent\")\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_target_not_found_anywhere_in_the_extends_ancestry() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    GoToState(\"StillMissing\")\nEndFunction\n",
            &mut FakeExternalWithAncestorState,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'StillMissing'"));
}

#[test]
fn walks_an_indexed_argument_without_crashing() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(String[] names, Int i)\n    Debug.Trace(names[i])\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_call_nested_in_a_named_argument_and_walks_a_new_array_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    SomeCall(flag = GoToState(\"Missing\"))\n    Int[] arr = new Int[3]\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Missing'"));
}

#[test]
fn does_not_treat_a_bare_self_call_as_goto_state() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Self(\"Missing\")\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn literal_goto_state_targets_keeps_only_self_literal_calls() {
    let source = r#"
ScriptName Example

Function Test(Example other, String target)
    GoToState("Busy")
    self.GoToState("Idle")
    GoToState("")
    other.GoToState("Ignored")
    GoToState(target)
EndFunction
"#;
    let script = papyrus_parser::parse(source).expect("script should parse");
    let targets = super::literal_goto_state_targets(&script);

    assert_eq!(
        targets,
        ["busy".to_string(), "idle".to_string()]
            .into_iter()
            .collect()
    );
}

#[test]
fn fake_external_with_ancestor_state_lookup_always_returns_none() {
    assert!(FakeExternalWithAncestorState
        .lookup("BaseScript", "SomeFunction")
        .is_none());
}

#[test]
fn line_and_file_disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\") ; @disable goto-state\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file goto-state\nScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn check_with_returns_no_diagnostics_without_an_ast() {
    let diagnostics = super::check_with(None, &mut FakeExternalWithAncestorState);

    assert!(diagnostics.is_empty());
}

#[test]
fn check_with_walks_variable_initializers_assignments_and_returns() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test()
    Bool initialized = GoToState("FromInitializer")
    initialized = GoToState("FromAssignment")
    Return GoToState("FromReturn")
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("'FromInitializer'"));
    assert!(diagnostics[1].message.contains("'FromAssignment'"));
    assert!(diagnostics[2].message.contains("'FromReturn'"));
}

#[test]
fn check_with_walks_conditions_and_every_control_flow_body() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test()
    If GoToState("FromIfCondition")
        GoToState("FromIfBody")
    ElseIf GoToState("FromElseIfCondition")
        GoToState("FromElseIfBody")
    Else
        GoToState("FromElseBody")
    EndIf
    While GoToState("FromWhileCondition")
        GoToState("FromWhileBody")
    EndWhile
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 7);
}

#[test]
fn check_with_walks_calls_nested_in_compound_expressions() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test(Int[] values, Int index)
    Bool binary = GoToState("Binary") == false
    Bool unary = !GoToState("Unary")
    Int casted = GoToState("Cast") as Int
    Int indexed = values[GoToState("Index")]
    Int[] allocated = new Int[GoToState("ArraySize")]
    SomeCall(named = GoToState("NamedArgument"), GoToState("CallArgument"))
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 7);
}

#[test]
fn check_with_ignores_non_string_and_wrong_arity_calls() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test(String target)
    GoToState(target)
    GoToState("One", "Two")
    Other.GoToState("Missing")
EndFunction
"#,
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}
