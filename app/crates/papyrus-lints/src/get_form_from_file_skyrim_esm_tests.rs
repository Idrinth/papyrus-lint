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
fn flags_formid_passed_positionally() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Skyrim.esm\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Game.GetForm"));
}

#[test]
fn flags_formid_after_the_file_name_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(\"Skyrim.esm\", 0x12345)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_named_arguments_in_either_order() {
    let in_order = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = 0x12345, asFileName = \"Skyrim.esm\")\nEndFunction\n",
    );
    let reordered = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(asFileName = \"Skyrim.esm\", auiFormID = 0x12345)\nEndFunction\n",
    );

    assert_eq!(in_order.len(), 1);
    assert_eq!(reordered.len(), 1);
}

#[test]
fn matches_game_function_and_file_name_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = gAmE.gEtFoRmFrOmFiLe(0x12345, \"SKYRIM.ESM\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_runtime_formid_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int aiFormID)\n    Form theForm = Game.GetFormFromFile(aiFormID, \"Skyrim.esm\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_different_plugin_file() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Update.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_file_name_that_only_contains_skyrim_esm() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Skyrim.esm.dll\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unqualified_get_form_from_file() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = GetFormFromFile(0x12345, \"Skyrim.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_same_named_function_on_an_unrelated_script() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(MyScript akOther)\n    akOther.GetFormFromFile(0x12345, \"Skyrim.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_missing_the_file_name_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345)\nEndFunction\n",
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
fn repairs_formid_passed_positionally() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Skyrim.esm\")\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetForm(0x12345)\nEndFunction\n"
    );
    assert!(check(&repaired).is_empty());
}

#[test]
fn repairs_formid_after_the_file_name_argument() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(\"Skyrim.esm\", 0x12345)\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetForm(0x12345)\nEndFunction\n"
    );
}

#[test]
fn repairs_named_arguments_dropping_their_names() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(asFileName = \"Skyrim.esm\", auiFormID = 0x12345)\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetForm(0x12345)\nEndFunction\n"
    );
}

#[test]
fn repairs_keeping_a_larger_formid_expression_intact() {
    let source =
        "ScriptName Example\n\nFunction Test(Int aiOffset)\n    Form theForm = Game.GetFormFromFile(0x12345 + aiOffset, \"Skyrim.esm\")\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test(Int aiOffset)\n    Form theForm = Game.GetForm(0x12345 + aiOffset)\nEndFunction\n"
    );
}

#[test]
fn repairs_every_flagged_call_on_the_same_line() {
    let source = "ScriptName Example\n\nFunction Test()\n    Form theA = Game.GetFormFromFile(0x1, \"Skyrim.esm\")\n    Form theB = Game.GetFormFromFile(0x2, \"Skyrim.esm\")\nEndFunction\n";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nFunction Test()\n    Form theA = Game.GetForm(0x1)\n    Form theB = Game.GetForm(0x2)\nEndFunction\n"
    );
}

#[test]
fn does_not_change_source_with_no_matching_call() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Update.esm\")\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_does_not_crash_on_unparseable_source() {
    let source = "ScriptName Example\n\nFunction Test(\nEndFunction\n";
    assert_eq!(repair(source), source);
}
