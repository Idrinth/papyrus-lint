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
fn flags_mismatched_return_value() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return \"hi\"\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'Test'"));
    assert!(diagnostics[0].message.contains("declares return type Int"));
    assert!(diagnostics[0].message.contains("returns String"));
}

#[test]
fn allows_matching_return_value() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_int_returned_from_float_function() {
    let diagnostics =
        check("ScriptName Example\n\nFloat Function Test()\n    Return 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_functions_with_no_declared_return_type() {
    let diagnostics = check("ScriptName Example\n\nFunction Test()\n    Return 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_bare_return_in_a_typed_function() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    If true\n        Return\n    EndIf\n    Return 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_none_returned_from_a_primitive_function() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return None\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("returns None"));
}

#[test]
fn allows_none_returned_from_an_object_typed_function() {
    let diagnostics =
        check("ScriptName Example\n\nActor Function Test()\n    Return None\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_none_returned_from_an_array_typed_function() {
    let diagnostics =
        check("ScriptName Example\n\nInt[] Function Test()\n    Return None\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_returns_nested_in_if_and_while_blocks() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return \"hi\"\n    Else\n        While flag\n            Return \"bye\"\n        EndWhile\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn does_not_flag_unresolvable_return_value() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Helper()\n    Return 1\nEndFunction\n\nInt Function Test()\n    Return Helper()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nInt Function Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_getitemcount_returned_from_a_bool_function() {
    let diagnostics = check_with(
        r#"ScriptName Example

Bool Function HasEnoughGold(Actor akActor, Int amount)
    Return akActor.GetItemCount(Gold001)
EndFunction
"#,
        &mut FakeExternalWithItemCount,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'HasEnoughGold'"));
    assert!(diagnostics[0].message.contains("declares return type Bool"));
    assert!(diagnostics[0].message.contains("returns Int"));
}

#[test]
fn allows_getitemcount_compared_against_the_required_amount() {
    let diagnostics = check_with(
        r#"ScriptName Example

Bool Function HasEnoughGold(Actor akActor, Int amount)
    Return akActor.GetItemCount(Gold001) >= amount
EndFunction
"#,
        &mut FakeExternalWithItemCount,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_local_call_returned_with_the_wrong_type() {
    let diagnostics = check(
        r#"ScriptName Example

Int Function Helper()
    Return 1
EndFunction

Bool Function Test()
    Return Helper()
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Test'"));
    assert!(diagnostics[0].message.contains("declares return type Bool"));
    assert!(diagnostics[0].message.contains("returns Int"));
}


struct FakeExternalWithItemCount;
struct FakeExternalWithSubtypes;

impl ExternalSignatures for FakeExternalWithItemCount {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn function_return_type(
        &mut self,
        type_name: &str,
        function_name: &str,
    ) -> Option<papyrus_parser::ast::TypeName> {
        let actor_like = type_name.eq_ignore_ascii_case("Actor")
            || type_name.eq_ignore_ascii_case("ObjectReference");
        if actor_like && function_name.eq_ignore_ascii_case("GetItemCount") {
            return Some(papyrus_parser::ast::TypeName {
                name: "Int".to_string(),
                is_array: false,
            });
        }
        None
    }
}

impl ExternalSignatures for FakeExternalWithSubtypes {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case("Armor") && super_type.eq_ignore_ascii_case("Form")
    }
}

#[test]
fn accepts_a_return_value_whose_script_extends_the_declared_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nForm Function Test()\n    Return MyArmor\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn check_with_returns_no_diagnostics_without_an_ast() {
    let diagnostics = super::check_with(None, &mut FakeExternalWithSubtypes);

    assert!(diagnostics.is_empty());
}

#[test]
fn check_with_checks_state_functions_and_nested_control_flow() {
    let diagnostics = check_with(
        r#"ScriptName Example

State Active
    Int Function Test(Bool flag)
        Int value = 1
        value = 2
        value
        If flag
            Return "if"
        ElseIf !flag
            Return
        Else
            While flag
                Return "while"
            EndWhile
        EndIf
    EndFunction
EndState
"#,
        &mut FakeExternalWithSubtypes,
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 9);
    assert_eq!(diagnostics[1].line, 14);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message.contains("'Test'")));
}

#[test]
fn still_flags_an_unrelated_object_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nForm Function Test()\n    Return MyWeapon\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("declares return type Form"));
    assert!(diagnostics[0].message.contains("returns Weapon"));
}
