use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
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
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
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
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
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
