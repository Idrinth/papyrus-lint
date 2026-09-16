use super::*;

#[test]
fn flags_cast_to_the_values_exact_type() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as Actor)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("'Actor'"));
}

#[test]
fn does_not_flag_a_cast_without_external_ancestor_resolution() {
    // `check` (no external resolver) can't tell that `Actor` extends
    // `ObjectReference`, so it leaves this unflagged; see
    // `check_with` below for the ancestor case.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as ObjectReference)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

struct FakeExternalWithSubtypes;

impl ExternalSignatures for FakeExternalWithSubtypes {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::argument_types::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case("Actor") && super_type.eq_ignore_ascii_case("ObjectReference")
    }
}

#[test]
fn flags_cast_to_a_known_ancestor_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as ObjectReference)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("'Actor' already extends 'ObjectReference'"));
}

#[test]
fn does_not_flag_cast_to_an_unrelated_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as Weapon)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_narrowing_cast_to_an_unrelated_subtype() {
    // `Weapon` doesn't extend `Actor` (nor vice versa per the fake
    // resolver), so this is a legitimate, potentially-narrowing cast.
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Foo(akRef as Actor)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_implicit_widening_between_primitives() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int a)\n    Foo(a as Float)\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_cast_to_the_same_primitive_type() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int a)\n    Foo(a as Int)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Int'"));
}

#[test]
fn does_not_flag_cast_whose_value_type_is_unresolvable() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Foo(GetTarget() as Actor)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_cast_on_a_property() {
    let diagnostics = check(
            "ScriptName Example\n\nActor Property MyActor Auto\n\nFunction Test()\n    Foo(MyActor as Actor)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Actor akActor)\n        Foo(akActor as Actor)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_a_cast_from_an_array_typed_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor[] actors)\n    Foo(actors as Actor)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn walks_a_cast_value_nested_in_an_index_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Actor[] arr)\n    Foo(arr[0] as Actor)\nEndFunction\n",
    );

    // Whether the array-element access itself resolves to a known type
    // isn't the point here; this just needs to walk the Index
    // expression nested inside the cast without crashing.
    assert!(diagnostics.len() <= 1);
}

#[test]
fn flags_a_cast_nested_in_a_named_argument_and_walks_a_new_array_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    SomeCall(flag = akActor as Actor)\n    Int[] arr = new Int[3]\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Actor'"));
}

#[test]
fn fake_external_with_subtypes_lookup_always_returns_none() {
    assert!(FakeExternalWithSubtypes
        .lookup("Actor", "IsGlobal")
        .is_none());
}
