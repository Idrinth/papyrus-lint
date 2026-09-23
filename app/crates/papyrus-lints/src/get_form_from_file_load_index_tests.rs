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
fn flags_a_non_zero_load_index_on_a_full_plugin() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x01012345, \"Update.esm\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("load index"));
    assert!(diagnostics[0].message.contains("0x12345"));
}

#[test]
fn flags_a_non_zero_load_index_on_a_light_plugin() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x1ABC, \"Light.esl\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("load index"));
    assert!(diagnostics[0].message.contains("0xFFF"));
    assert!(diagnostics[0].message.contains("0xABC"));
}

#[test]
fn flags_leading_zero_padding_on_a_full_plugin() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x00012345, \"Update.esm\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("leading zero"));
    assert!(diagnostics[0].message.contains("0x12345"));
}

#[test]
fn flags_leading_zero_padding_on_a_light_plugin() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x00F, \"Light.esl\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("leading zero"));
}

#[test]
fn does_not_flag_a_minimally_written_full_plugin_formid() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Update.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_minimally_written_light_plugin_formid() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0xABC, \"Light.esl\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_light_plugin_formid_that_would_be_fine_on_a_full_plugin() {
    // 0xABCD is out of range for a light plugin (max 0xFFF) but this is
    // exactly the case this rule exists to catch, so it must still flag.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0xABCD, \"Light.esl\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("load index"));
}

#[test]
fn flags_named_arguments_in_either_order() {
    let in_order = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = 0x01012345, asFileName = \"Update.esm\")\nEndFunction\n",
    );
    let reordered = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(asFileName = \"Update.esm\", auiFormID = 0x01012345)\nEndFunction\n",
    );

    assert_eq!(in_order.len(), 1);
    assert_eq!(reordered.len(), 1);
}

#[test]
fn matches_game_function_and_file_extension_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = gAmE.gEtFoRmFrOmFiLe(0x1ABC, \"Light.ESL\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("0xFFF"));
}

#[test]
fn does_not_flag_a_decimal_formid() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(16843845, \"Update.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_runtime_formid_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int aiFormID)\n    Form theForm = Game.GetFormFromFile(aiFormID, \"Update.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_runtime_file_name_expression() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(String asFile)\n    Form theForm = Game.GetFormFromFile(0x01012345, asFile)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unqualified_get_form_from_file() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = GetFormFromFile(0x01012345, \"Update.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_same_named_function_on_an_unrelated_script() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(MyScript akOther)\n    akOther.GetFormFromFile(0x01012345, \"Update.esm\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_call_missing_the_file_name_argument() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x01012345)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
