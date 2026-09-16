use super::*;

#[test]
fn flags_inline_cast_dereferenced_directly() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    (akRef as Actor).GetActorValue(\"Health\")\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'Actor'"));
    assert!(diagnostics[0].message.contains(".GetActorValue"));
}

#[test]
fn flags_method_call_on_variable_assigned_a_cast() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    a.GetActorValue(\"Health\")\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains("'a'"));
}

#[test]
fn flags_property_access_on_variable_assigned_a_cast() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a\n    a = akRef as Actor\n    Debug.Trace(a.Name)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_after_early_return_none_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If a == None\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_bang_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If !a\n        Return\n    EndIf\n    a.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_inside_not_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If a != None\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_inside_and_guarded_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Bool flag)\n    Actor a = akRef as Actor\n    If a != None && flag\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_inside_equal_none_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    If a == None\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

    // Evaluating the check clears "unchecked" regardless of branch, so
    // this is left to `none-form-usage` (a different, more precise
    // lint) to flag instead.
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_while_loop_condition_checks_it() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    While a == None\n        a = akRef as Actor\n    EndWhile\n    a.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_variable_reassigned_from_a_non_cast_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Actor akActor)\n    Actor a = akRef as Actor\n    a = akActor\n    a.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_declaration_without_a_cast() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Actor a = akActor\n    a.GetName()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_passing_an_unchecked_cast_variable_as_an_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    Actor a = akRef as Actor\n    Debug.Trace(a)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_use_still_unchecked_after_one_sided_reassignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Actor akActor, Bool flag)\n    Actor a = akActor\n    If flag\n        a = akRef as Actor\n    EndIf\n    a.GetName()\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn does_not_flag_when_both_branches_check_it() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(ObjectReference akRef, Bool flag)\n    Actor a = akRef as Actor\n    If flag && a != None\n        a.GetName()\n    ElseIf a != None\n        a.GetName()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(ObjectReference akRef)\n        Actor a = akRef as Actor\n        a.GetName()\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_inline_cast_guarded_by_the_same_cast_in_an_and_chain() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Form akSource)\n    If (akSource as Weapon || (akSource as Spell && (akSource as Spell).isHostile()))\n        Debug.Trace(\"hostile\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_inline_cast_guarded_by_not_equal_none_in_an_and_chain() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Form akSource)\n    If (akSource as Spell != None && (akSource as Spell).isHostile())\n        Debug.Trace(\"hostile\")\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_inline_cast_when_the_and_guard_is_a_different_cast_target() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Form akSource, Form akOther)\n    If (akOther as Spell && (akSource as Spell).isHostile())\n        Debug.Trace(\"hostile\")\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Spell'"));
}

#[test]
fn still_flags_inline_cast_guarded_only_on_the_or_side() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Form akSource)\n    If (akSource as Spell == None || (akSource as Spell).isHostile())\n        Debug.Trace(\"hostile\")\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_the_creation_kit_generated_cast_in_a_fragment_wrapper() {
    let diagnostics = check(
        "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
;NEXT FRAGMENT INDEX 0
Scriptname IDR__TIF__05000235 Extends TopicInfo Hidden

;BEGIN FRAGMENT Fragment_0
Function Fragment_0(ObjectReference akSpeakerRef)
Actor akSpeaker = akSpeakerRef as Actor
;BEGIN CODE
akSpeaker.RemoveItem(idrinthAlyienethMikaelsSong, 1, false, PlayerRef)
PlayerRef.RemoveItem(Gold001, 5, false, akSpeaker)
;END CODE
EndFunction
;END FRAGMENT

;END FRAGMENT CODE - Do not edit anything between this and the begin comment
Actor Property PlayerRef  Auto
",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_the_same_cast_pattern_outside_a_fragment_wrapper() {
    let diagnostics = check(
            "ScriptName Example Extends TopicInfo Hidden\n\nFunction Fragment_0(ObjectReference akSpeakerRef)\n    Actor akSpeaker = akSpeakerRef as Actor\n    akSpeaker.RemoveItem(Gold001, 1, false, PlayerRef)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'akSpeaker'"));
}
