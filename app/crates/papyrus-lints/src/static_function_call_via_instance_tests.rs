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

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}

#[test]
fn does_not_flag_anything_without_a_resolver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.SomeGlobal()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("MyScript")
            && function_name.eq_ignore_ascii_case("IsGlobal")
        {
            Some(true)
        } else if type_name.eq_ignore_ascii_case("MyScript")
            && function_name.eq_ignore_ascii_case("NotGlobal")
        {
            Some(false)
        } else {
            None
        }
    }
}

#[test]
fn flags_a_global_function_called_through_a_local_variable() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.IsGlobal()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("'IsGlobal' is declared Global on 'MyScript'"));
}

#[test]
fn flags_a_global_function_called_through_a_property() {
    let diagnostics = check_with(
            "ScriptName Example\n\nMyScript Property Target Auto\n\nFunction Test()\n    Target.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_global_function_called_through_an_array_element() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript[] refs, Int i)\n    refs[i].IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 21);
}

#[test]
fn flags_a_global_function_called_through_a_cast() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Form value)\n    (value as MyScript).IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("calling it as 'MyScript.IsGlobal()'"));
}

#[test]
fn does_not_flag_a_call_on_an_array_itself() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(MyScript[] refs)\n    refs.IsGlobal()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_instance_function_called_through_a_local_variable() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.NotGlobal()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_through_self_or_parent() {
    let diagnostics = check_with(
            "ScriptName Example Extends MyScript\n\nFunction Test()\n    self.IsGlobal()\n    parent.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unresolved_object_or_function() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    akRef.SomethingElse()\n    MyScript.IsGlobal()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn finds_a_call_in_nested_control_flow_and_state_functions() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test(MyScript akRef)
    If true
        akRef.IsGlobal()
    EndIf
EndFunction

State Active
    Function Run(MyScript akRef)
        akRef.IsGlobal()
    EndFunction
EndState
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn finds_calls_in_each_statement_and_expression_position() {
    let diagnostics = check_with(
        r#"ScriptName Example

Function Test(MyScript akRef)
    Bool initialized = akRef.IsGlobal()
    initialized = akRef.IsGlobal()
    If akRef.IsGlobal() && !akRef.IsGlobal()
    ElseIf akRef.IsGlobal()
    EndIf
    While akRef.IsGlobal()
        SomeCall(akRef.IsGlobal())
    EndWhile
    Return akRef.IsGlobal()
EndFunction
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 8);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule == RULE));
}

#[test]
fn resolves_type_and_function_names_case_insensitively() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(myscript akRef)\n    akRef.isglobal()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("'isglobal' is declared Global on 'myscript'"));
}

#[test]
fn walks_an_indexed_receiver_used_as_a_plain_call_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript[] refs, Int i)\n    Debug.Trace(refs[i])\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_call_nested_inside_a_named_argument_and_walks_a_new_array_expression() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(MyScript akRef)\n    SomeCall(flag = akRef.IsGlobal())\n    Int[] arr = new Int[3]\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("IsGlobal"));
}

#[test]
fn fake_external_lookup_always_returns_none() {
    assert!(FakeExternal.lookup("MyScript", "IsGlobal").is_none());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}
