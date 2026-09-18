use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_the_same_outfit_applied_twice_in_a_row() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn flags_a_repeat_with_unrelated_statements_between() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    Debug.Trace(\"hi\")\n    Int a = 1\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn does_not_flag_a_single_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_different_outfits() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB)\n    akActor.SetOutfit(OutfitA)\n    akActor.SetOutfit(OutfitB)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_the_same_outfit_after_a_different_outfit_was_applied_between() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB)\n    akActor.SetOutfit(OutfitA)\n    akActor.SetOutfit(OutfitB)\n    akActor.SetOutfit(OutfitA)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_different_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActorA, Actor akActorB, Outfit MyOutfit)\n    akActorA.SetOutfit(MyOutfit)\n    akActorB.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_different_sleep_outfit_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    akActor.SetOutfit(MyOutfit, true)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unqualified_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Outfit MyOutfit)\n    SetOutfit(MyOutfit)\n    SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_different_method_name() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfitDefault(MyOutfit)\n    akActor.SetOutfitDefault(MyOutfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_the_method_name_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.setoutfit(MyOutfit)\n    akActor.SETOUTFIT(MyOutfit)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_repeat_nested_inside_an_if_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Bool bReady)\n    akActor.SetOutfit(MyOutfit)\n    If bReady\n        akActor.SetOutfit(MyOutfit)\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_carry_a_conditional_change_past_the_if_statement() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB, Bool bReady)\n    akActor.SetOutfit(OutfitA)\n    If bReady\n        akActor.SetOutfit(OutfitB)\n    EndIf\n    akActor.SetOutfit(OutfitA)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn checks_each_if_branch_independently_of_its_siblings() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Bool bReady)\n    If bReady\n        akActor.SetOutfit(MyOutfit)\n    Else\n        akActor.SetOutfit(MyOutfit)\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_repeat_nested_inside_a_while_loop() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Bool bReady)\n    akActor.SetOutfit(MyOutfit)\n    While bReady\n        akActor.SetOutfit(MyOutfit)\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Waiting\n    Function Test(Actor akActor, Outfit MyOutfit)\n        akActor.SetOutfit(MyOutfit)\n        akActor.SetOutfit(MyOutfit)\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn resolves_self_and_chained_member_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Outfit MyOutfit)\n    Self.SetOutfit(MyOutfit)\n    Self.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_receiver_and_argument_identifiers_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    AKACTOR.SetOutfit(MYOUTFIT)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_after_the_outfit_variable_is_reassigned() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit OutfitA, Outfit OutfitB)\n    Outfit outfit = OutfitA\n    akActor.SetOutfit(outfit)\n    outfit = OutfitB\n    akActor.SetOutfit(outfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_repeat_separated_by_an_unrelated_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit, Int a)\n    akActor.SetOutfit(MyOutfit)\n    a = 1\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn stops_scanning_after_a_return() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    Return\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
