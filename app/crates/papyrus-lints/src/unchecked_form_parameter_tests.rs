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
fn flags_method_call_on_unchecked_form_parameter() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetName()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'akArmor'"));
    assert!(diagnostics[0].message.contains(".GetName"));
}

#[test]
fn flags_property_access_on_unchecked_form_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Debug.Trace(akArmor.Name)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_primitive_or_array_parameters() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int i, Bool b, String s, Armor[] arr)\n    Debug.Trace(s)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_early_return_none_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor == None\n        Return\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_bang_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If !akArmor\n        Return\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_inside_not_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor != None\n        akArmor.GetName()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_inside_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor == None\n        akArmor.GetName()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_reassigned_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor = Game.GetPlayer() as Armor\n    akArmor.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_passing_an_unchecked_parameter_as_an_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Debug.Trace(akArmor)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_use_still_possibly_unchecked_after_one_sided_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor, Bool flag)\n    If flag\n        If akArmor == None\n            Return\n        EndIf\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

#[test]
fn does_not_flag_after_while_loop_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    While akArmor == None\n        akArmor = Game.GetPlayer() as Armor\n    EndWhile\n    akArmor.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Armor akArmor)\n        akArmor.GetName()\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_short_circuited_and_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor && akArmor.GetName() == \"\"\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_local_variable_narrowed_via_short_circuit_and() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Armor localArmor\n    If localArmor == None && localArmor.GetName() == \"\"\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_access_in_a_var_decl_initializer() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    String name = akArmor.GetName()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_access_in_a_return_value() {
    let diagnostics = check(
            "ScriptName Example\n\nString Function Test(Armor akArmor)\n    Return akArmor.GetName()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_bare_return_with_no_value() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Armor akArmor)\n    Return\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn stays_unchecked_after_an_if_else_chain_that_returns_on_every_path() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor, Bool flag)\n    If flag\n        Return\n    Else\n        Return\n    EndIf\n    akArmor.GetName()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

#[test]
fn flags_only_the_innermost_direct_access_in_a_chained_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetContainer().GetName()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_short_circuited_or_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    If akArmor == None || akArmor.GetName() == \"\"\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_the_right_side_of_an_unrelated_or_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor, Bool flag)\n    If flag || akArmor.GetName() == \"\"\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_access_nested_in_an_index_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor, Int[] arr)\n    Int x = arr[akArmor.GetName().Length]\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_access_nested_in_a_cast_and_a_new_array_size_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Armor akArmor)\n    Form f = akArmor.GetLinkedRef() as Form\n    Int[] arr = new Int[akArmor.GetGoldValue()]\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
}
