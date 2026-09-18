use super::*;
use crate::{lint, repair_filtered, Config};

fn script(body: &str) -> String {
    format!("ScriptName Example\n\nFunction Test()\n    {body}\nEndFunction\n")
}

#[test]
fn flags_positional_skyrim_esm_call() {
    let diagnostics = check(&script(
        "Form f = Game.GetFormFromFile(0x12C87, \"Skyrim.esm\")",
    ));
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Game.GetForm(x)"));
}

#[test]
fn flags_case_insensitive_filename_and_qualifier() {
    let diagnostics = check(&script(
        "Form f = game.getformfromfile(42, \"skyrim.ESM\")",
    ));
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_named_arguments() {
    let diagnostics = check(&script(
        "Form f = Game.GetFormFromFile(asFilename = \"Skyrim.esm\", aiFormID = 0x12C87)",
    ));
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn ignores_other_plugins() {
    let diagnostics = check(&script(
        "Form f = Game.GetFormFromFile(0x12C87, \"Dawnguard.esm\")",
    ));
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_filename_in_a_variable() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String file = \"Skyrim.esm\"\n    Form f = Game.GetFormFromFile(0x12C87, file)\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_unqualified_or_non_game_receiver() {
    let diagnostics = check(&script(
        "Form a = GetFormFromFile(0x1, \"Skyrim.esm\")\n    Form b = Other.GetFormFromFile(0x1, \"Skyrim.esm\")",
    ));
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_string_literals_in_comments() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    ; Game.GetFormFromFile(0x1, \"Skyrim.esm\")\n    String text = \"Game.GetFormFromFile(0x1, \\\"Skyrim.esm\\\")\"\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn repair_rewrites_positional_call() {
    let source = script("Form f = Game.GetFormFromFile(0x12C87, \"Skyrim.esm\")");
    let repaired = repair(&source);
    assert!(repaired.contains("Game.GetForm(0x12C87)"));
    assert!(!repaired.contains("GetFormFromFile"));
}

#[test]
fn repair_rewrites_named_call_keeping_form_id_expression() {
    let source = script(
        "Form f = Game.GetFormFromFile(asFilename = \"Skyrim.esm\", aiFormID = someId)",
    );
    let repaired = repair(&source);
    assert!(repaired.contains("Game.GetForm(someId)"));
    assert!(!repaired.contains("GetFormFromFile"));
}

#[test]
fn disable_comment_suppresses_the_rule() {
    let source = script(
        "Form f = Game.GetFormFromFile(0x12C87, \"Skyrim.esm\") ; @disable getform-from-skyrim-esm",
    );
    let diagnostics = lint(&source, &Config::default());
    assert!(diagnostics.iter().all(|d| d.rule != RULE));
}

#[test]
fn filtered_repair_only_touches_this_rule() {
    let source = script("Form f = Game.GetFormFromFile(0x12C87, \"Skyrim.esm\")");
    let repaired = repair_filtered(&source, &Config::default(), Some(RULE));
    assert!(repaired.contains("Game.GetForm(0x12C87)"));
}
