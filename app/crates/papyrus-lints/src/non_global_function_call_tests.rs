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

fn check_with_visitor<E: ExternalSignatures>(
    source: &str,
    config: &crate::config::Config,
    external: &mut E,
) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(source, ast.as_ref(), tokens.as_deref(), config, external)
}

fn lint_with<E: ExternalSignatures>(
    source: &str,
    config: &crate::config::Config,
    external: &mut E,
) -> Vec<Diagnostic> {
    crate::lint_with_external_arguments(source, config, external)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == RULE)
        .collect()
}

#[test]
fn does_not_flag_anything_without_a_resolver() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    MyScript.NotGlobal()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("MyScriptOne")
            && function_name.eq_ignore_ascii_case("IMNotStatic")
        {
            Some(false)
        } else if type_name.eq_ignore_ascii_case("Utility")
            && function_name.eq_ignore_ascii_case("Wait")
        {
            Some(true)
        } else {
            None
        }
    }
}

#[test]
fn flags_a_call_through_a_script_name_to_a_non_global_function() {
    let diagnostics = check_with(
            "ScriptName MyScriptTwo\n\nFunction Mine()\n    Return MyScriptOne.IMNotStatic()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("'IMNotStatic' is not declared Global on 'MyScriptOne'"));
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 35);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn visitor_flags_a_non_global_call() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    MyScriptOne.IMNotStatic()\nEndFunction\n";

    let diagnostics = check_with_visitor(
        source,
        &crate::config::Config::default(),
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn disable_comment_suppresses_the_visitor_finding() {
    let source = r#"ScriptName Example

Function Test()
    MyScriptOne.IMNotStatic()
    MyScriptOne.IMNotStatic() ; @disable non-global-function-call
EndFunction
"#;

    let diagnostics = lint_with(
        source,
        &crate::config::Config::default(),
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn visitor_honors_the_config_off_switch() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    MyScriptOne.IMNotStatic()\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.non_global_function_call = false;

    let diagnostics = lint_with(source, &config, &mut FakeExternal);

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_to_a_known_global_function() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(1.0)\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unresolved_script_or_function() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test()\n    MyMissingScript.DoThing()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_through_a_local_variable_property_or_self() {
    let diagnostics = check_with(
        r#"
ScriptName Example

MyScriptOne Property Target Auto

Function Test(MyScriptOne akRef)
    akRef.IMNotStatic()
    Target.IMNotStatic()
    self.Test(None)
EndFunction
"#,
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn finds_a_non_global_call_in_nested_control_flow_and_state_functions() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test()
    If true
        MyScriptOne.IMNotStatic()
    EndIf
EndFunction

State Active
    Function Run()
        MyScriptOne.IMNotStatic()
    EndFunction
EndState
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn finds_calls_in_every_statement_and_nested_expression_position() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test()
    Bool result = MyScriptOne.IMNotStatic()
    result = MyScriptOne.IMNotStatic()
    If MyScriptOne.IMNotStatic() && !MyScriptOne.IMNotStatic()
        result = (MyScriptOne.IMNotStatic() as Bool)
    ElseIf MyScriptOne.IMNotStatic()
        MyScriptOne.IMNotStatic()
    Else
        While MyScriptOne.IMNotStatic()
            Int[] values = new Int[MyScriptOne.IMNotStatic()]
            values[MyScriptOne.IMNotStatic()] = 1
        EndWhile
    EndIf
    Return MyScriptOne.IMNotStatic()
EndFunction
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 11);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule == RULE));
}

#[test]
fn ignores_direct_calls_and_calls_on_non_identifier_expressions() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test(MyScriptOne[] refs)
    Test(refs)
    refs[0].IMNotStatic()
    GetTarget().IMNotStatic()
EndFunction

MyScriptOne Function GetTarget()
EndFunction
"#,
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());

    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(\nEndFunction\n",
        &mut FakeExternal,
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn walks_an_indexed_argument_without_crashing() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScriptOne[] refs, Int i)\n    Debug.Trace(refs[i])\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_call_nested_in_a_named_argument_and_walks_a_new_array_expression() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test()\n    SomeCall(flag = MyScriptOne.IMNotStatic())\n    Int[] arr = new Int[3]\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("IMNotStatic"));
}

#[test]
fn fake_external_lookup_always_returns_none() {
    assert!(FakeExternal.lookup("MyScriptOne", "IMNotStatic").is_none());
}
