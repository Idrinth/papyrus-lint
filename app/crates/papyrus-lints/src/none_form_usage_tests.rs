use super::*;

fn check(source: &str, assume_auto_properties_filled: bool) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        assume_auto_properties_filled,
        ..Default::default()
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::argument_types::NoExternalSignatures,
    )
}

#[test]
fn flags_method_call_on_variable_declared_none() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
        false,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'a'"));
    assert!(diagnostics[0].message.contains(".GetName"));
}

#[test]
fn flags_property_access_on_variable_assigned_none() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a\n    a = None\n    Debug.Trace(a.Name)\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_variable_reassigned_before_use() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a = Game.GetPlayer() as Armor\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_early_return_none_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    If a == None\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_bang_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    If !a\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_inside_not_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    If a != None\n        a.GetName()\n    EndIf\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_inside_and_guarded_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Armor a = None\n    If a != None && flag\n        a.GetName()\n    EndIf\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_or_guarded_early_return() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Armor a = None\n    If a == None || flag\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_inside_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    If a == None\n        a.GetName()\n    EndIf\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn flags_use_still_possibly_none_after_one_sided_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Armor a = None\n    If flag\n        a = Game.GetPlayer() as Armor\n    EndIf\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn does_not_flag_when_both_branches_assign_non_none() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Armor a = None\n    If flag\n        a = Game.GetPlayer() as Armor\n    Else\n        a = Game.GetPlayer() as Armor\n    EndIf\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_while_loop_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    While a == None\n        a = Game.GetPlayer() as Armor\n    EndWhile\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_short_circuited_method_call_in_while_condition() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    Int c = 0\n    While a == None || a.IsDead()\n        a = Game.GetPlayer()\n        c += 1\n    EndWhile\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_passing_a_possibly_none_variable_as_an_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    Debug.Trace(a)\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_uninitialized_form_declaration() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Armor a\n    a.GetName()\nEndFunction\n",
        false,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_uninitialized_declaration_after_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a\n    a = Game.GetPlayer() as Armor\n    a.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_uninitialized_primitive_declarations() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int i\n    Float f\n    Bool b\n    String s\n    Debug.Trace(s)\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_uninitialized_array_declaration() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Armor[] a\n    Debug.Trace(a)\nEndFunction\n",
        false,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Armor a = None\n        a.GetName()\n    EndFunction\nEndState\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_use_after_aliasing_a_still_none_variable() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    Armor b\n    b = a\n    b.GetName()\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
    assert!(diagnostics[0].message.contains("'b'"));
}

#[test]
fn does_not_flag_aliasing_a_known_not_none_variable() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Armor a = Game.GetPlayer() as Armor\n    Armor b = None\n    b = a\n    b.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unguarded_use_of_an_uninitialized_auto_property() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    MyArmor.GetName()\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("'MyArmor'"));
}

#[test]
fn flags_unguarded_use_of_an_auto_property_explicitly_defaulted_to_none() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor = None Auto\n\nFunction Test()\n    MyArmor.GetName()\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn assume_auto_properties_filled_suppresses_the_uninitialized_property_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    MyArmor.GetName()\nEndFunction\n",
            true,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn assume_auto_properties_filled_suppresses_the_explicit_none_default_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor = None Auto\n\nFunction Test()\n    MyArmor.GetName()\nEndFunction\n",
            true,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn assume_auto_properties_filled_still_flags_a_property_explicitly_set_to_none() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    MyArmor = None\n    MyArmor.GetName()\nEndFunction\n",
            true,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn assume_auto_properties_filled_still_flags_a_local_variable() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
        true,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_auto_property_guarded_by_a_none_check() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    If MyArmor != None\n        MyArmor.GetName()\n    EndIf\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_auto_property_reassigned_before_use() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test()\n    MyArmor = Game.GetPlayer() as Armor\n    MyArmor.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_auto_read_only_property_with_a_non_none_default() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor = Game.GetPlayer() as Armor AutoReadOnly\n\nFunction Test()\n    MyArmor.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_full_property_without_an_auto_keyword() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor\n    Armor Function Get()\n        Return None\n    EndFunction\nEndProperty\n\nFunction Test()\n    MyArmor.GetName()\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_uninitialized_primitive_auto_property() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyCount Auto\n\nFunction Test()\n    Debug.Trace(MyCount as String)\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn each_function_starts_fresh_for_a_default_none_property() {
    let diagnostics = check(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction First()\n    MyArmor = Game.GetPlayer() as Armor\nEndFunction\n\nFunction Second()\n    MyArmor.GetName()\nEndFunction\n",
            false,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n", false).is_empty());
}

#[test]
fn does_not_flag_short_circuited_and_guard_on_a_property() {
    let diagnostics = check(
            "Scriptname ShortCircuitProbe extends Quest\n\nQuest Property QA Auto\n\nFunction Probe()\n\tQA = None\n\tif (QA && QA.IsRunning())\n\t\tDebug.Trace(\"x\")\n\tendif\n\tif (QA != None)\n\t\tQA.SetStage(2)\n\tendif\nEndFunction\n",
            false,
        );

    assert!(diagnostics.is_empty());
}
