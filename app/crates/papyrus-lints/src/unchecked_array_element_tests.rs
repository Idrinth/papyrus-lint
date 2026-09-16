use super::*;

#[test]
fn flags_method_call_on_unchecked_array_element() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    act[2].Kill()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'act'"));
    assert!(diagnostics[0].message.contains('2'));
    assert!(diagnostics[0].message.contains(".Kill"));
}

#[test]
fn flags_property_access_on_unchecked_array_element() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    Debug.Trace(act[0].Name)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_primitive_array_elements() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    Bool[] b = new Bool[1]\n    Debug.Trace(a[0] as String)\n    Debug.Trace(b[0] as String)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_non_literal_index() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int i)\n    Actor[] act = new Actor[3]\n    act[i].Kill()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_property_array() {
    let diagnostics = check(
            "ScriptName Example\n\nActor[] Property MyActors Auto\n\nFunction Test()\n    MyActors[0].Kill()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_not_equal_none_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[2] != None\n        act[2].Kill()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_bare_truthy_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[2]\n        act[2].Kill()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_inside_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[2] == None\n        act[2].Kill()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_after_early_return_none_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[2] == None\n        Return\n    EndIf\n    act[2].Kill()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_bang_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If !act[2]\n        Return\n    EndIf\n    act[2].Kill()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_use_still_possibly_unchecked_after_one_sided_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Actor[] act = new Actor[3]\n    If flag\n        If act[2] == None\n            Return\n        EndIf\n    EndIf\n    act[2].Kill()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 10);
}

#[test]
fn does_not_flag_when_both_branches_confirm_the_element() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Actor[] act = new Actor[3]\n    If flag\n        If act[2] == None\n            Return\n        EndIf\n    Else\n        If act[2] == None\n            Return\n        EndIf\n    EndIf\n    act[2].Kill()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_while_loop_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    While act[2] == None\n        act[2] = Game.GetPlayer()\n    EndWhile\n    act[2].Kill()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_reassignment_that_could_reintroduce_none() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[2] != None\n        act[2] = None\n        act[2].Kill()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn tracks_an_object_array_parameter_too() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Actor[] act)\n    act[0].Kill()\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn constant_folding_normalizes_the_index_key() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[1 + 1] != None\n        act[2].Kill()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_passing_an_unchecked_element_as_an_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    Debug.Trace(act[0])\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_short_circuited_and_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[0] != None && act[0].IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_short_circuited_or_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    If act[0] == None || act[0].IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_the_right_side_of_an_unrelated_or_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Actor[] act = new Actor[3]\n    If flag || act[0].IsDead()\n        Debug.Trace(\"x\")\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Actor[] act = new Actor[3]\n        act[2].Kill()\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn each_function_starts_fresh() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction First()\n    Actor[] act = new Actor[3]\n    If act[2] != None\n        act[2].Kill()\n    EndIf\nEndFunction\n\nFunction Second()\n    Actor[] act = new Actor[3]\n    act[2].Kill()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 12);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
