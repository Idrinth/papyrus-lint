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

fn repair(source: &str) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
    )
}

#[test]
fn flags_decimal_formid_compared_after_get_form_id() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("76935"));
    assert!(diagnostics[0].message.contains("0x12C87"));
}

#[test]
fn flags_decimal_formid_compared_before_get_form_id() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If 76935 == akActor.GetFormID()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_hex_formid_in_either_order() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 0x00012C87\n        If 0X12c87 == akActor.GetFormID()\n        EndIf\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_every_comparison_operator() {
    let source = "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 1\n    EndIf\n    If akActor.GetFormID() != 2\n    EndIf\n    If akActor.GetFormID() > 3\n    EndIf\n    If akActor.GetFormID() < 4\n    EndIf\n    If akActor.GetFormID() >= 5\n    EndIf\n    If akActor.GetFormID() <= 6\n    EndIf\nEndFunction\n";

    assert_eq!(check(source).len(), 6);
}

#[test]
fn flags_unqualified_and_self_receivers() {
    let diagnostics = check(
            "ScriptName Example Extends Actor\n\nFunction Test()\n    If GetFormID() == 76935\n    EndIf\n    If Self.GetFormID() == 76935\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn flags_parent_receiver_and_case_insensitive_call_name() {
    let diagnostics = check(
            "ScriptName Example Extends Actor\n\nFunction Test()\n    If Parent.gEtFoRmId() != 42\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("0x2A"));
}

#[test]
fn flags_negative_decimal_after_get_form_id() {
    let diagnostics = check(
            "ScriptName Example Extends Actor\n\nFunction Test()\n    If GetFormID() == -1\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].column, 24);
}

#[test]
fn flags_negative_decimal_before_get_form_id() {
    let diagnostics = check(
            "ScriptName Example Extends Actor\n\nFunction Test()\n    If -1 <= Self.GetFormID()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 9);
    assert!(diagnostics[0].message.contains("decimal (1)"));
    assert!(diagnostics[0].message.contains("0x1"));
}

#[test]
fn flags_comparisons_against_chained_receivers() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript context)\n    If 42 != context.currentActor.GetFormID()\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_get_form_id_compared_to_a_runtime_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Int aiOther)\n    If akActor.GetFormID() == aiOther\n    EndIf\n    If akActor.GetFormID() == GetOtherFormID()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_two_get_form_id_calls_compared_to_each_other() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akA, Actor akB)\n    If akA.GetFormID() == akB.GetFormID()\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_get_form_id_used_outside_a_comparison() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Int id = akActor.GetFormID()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_get_form_id_with_arguments_as_the_target_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If GetFormID(1) == 42\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_target_names_in_comments_and_strings() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    ; GetFormID() == 42\n    String text = \"Game.GetFormFromFile(42)\"\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_decimal_formid_passed_positionally_to_get_form_from_file() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(76935, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("Game.GetFormFromFile"));
}

#[test]
fn flags_decimal_formid_passed_by_name_to_get_form_from_file() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = 76935, asPluginName = \"Skyrim.esm\")\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_game_and_function_names_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = gAmE.gEtFoRmFrOmFiLe(42, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("0x2A"));
}

#[test]
fn flags_negative_named_formid_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = -1)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_hex_formid_passed_to_get_form_from_file() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x00012C87, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_get_form_from_file_with_a_runtime_formid() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int aiFormID)\n    Form theForm = Game.GetFormFromFile(aiFormID, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_decimal_inside_a_larger_argument_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int aiOffset)\n    Form theForm = Game.GetFormFromFile(76935 + aiOffset, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unqualified_get_form_from_file() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = GetFormFromFile(76935, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_same_named_function_on_an_unrelated_script() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akOther)\n    akOther.GetFormFromFile(76935, \"Skyrim.esm\")\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_decimal_in_a_later_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int aiFormID)\n    Form theForm = Game.GetFormFromFile(aiFormID, 76935)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_incomplete_get_form_from_file_call() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Game.GetFormFromFile\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn repairs_decimal_formid_compared_after_get_form_id() {
    let source =
        "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 0x12C87\n    EndIf\nEndFunction\n"
    );
    assert!(check(&repaired).is_empty());
}

#[test]
fn repairs_decimal_formid_compared_before_get_form_id() {
    let source =
        "ScriptName Example\n\nFunction Test(Actor akActor)\n    If 76935 == akActor.GetFormID()\n    EndIf\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test(Actor akActor)\n    If 0x12C87 == akActor.GetFormID()\n    EndIf\nEndFunction\n"
    );
}

#[test]
fn repairs_negative_decimal_formid_leaving_the_minus_sign_alone() {
    let source =
        "ScriptName Example Extends Actor\n\nFunction Test()\n    If GetFormID() == -1\n    EndIf\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example Extends Actor\n\nFunction Test()\n    If GetFormID() == -0x1\n    EndIf\nEndFunction\n"
    );
}

#[test]
fn repairs_decimal_formid_passed_positionally_to_get_form_from_file() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(76935, \"Skyrim.esm\")\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12C87, \"Skyrim.esm\")\nEndFunction\n"
    );
}

#[test]
fn repairs_decimal_formid_passed_by_name_to_get_form_from_file() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = 76935, asPluginName = \"Skyrim.esm\")\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = 0x12C87, asPluginName = \"Skyrim.esm\")\nEndFunction\n"
    );
}

#[test]
fn repairs_every_flagged_literal_on_the_same_line() {
    let source =
        "ScriptName Example\n\nFunction Test(Actor akA, Actor akB)\n    If akA.GetFormID() == 76935 || akB.GetFormID() == 42\n    EndIf\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test(Actor akA, Actor akB)\n    If akA.GetFormID() == 0x12C87 || akB.GetFormID() == 0x2A\n    EndIf\nEndFunction\n"
    );
}

#[test]
fn does_not_change_source_with_no_decimal_formid() {
    let source =
        "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 0x00012C87\n    EndIf\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_does_not_crash_on_unparseable_source() {
    let source = "ScriptName Example\n\nFunction Test(\nEndFunction\n";
    assert_eq!(repair(source), source);
}
