//! Fallout 4's Papyrus dialect: custom `Struct`s, property `Group`s, and
//! the `DebugOnly`/`BetaOnly` function flags. All are opt-in through
//! [`GameEdition::Fallout4`]; [`parse`] (always Skyrim mode) rejects them
//! exactly as it would any other unrecognized construct.

use papyrus_parser::ast::Expr;
use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse, parse_with_mode, PapyrusError};

#[test]
fn parses_a_struct_declaration_with_typed_members_and_defaults() {
    let script = parse_with_mode(
        r#"ScriptName StructScript

Struct Coordinates
    Float X
    Float Y = 0.0
    Bool IsValid = True
EndStruct
"#,
        GameEdition::Fallout4,
    )
    .expect("a Fallout 4 struct declaration should parse");

    assert_eq!(script.structs.len(), 1);
    let coords = &script.structs[0];
    assert_eq!(coords.name, "Coordinates");
    assert_eq!(coords.members.len(), 3);
    assert_eq!(coords.members[0].type_name.name, "Float");
    assert_eq!(coords.members[0].name, "X");
    assert_eq!(coords.members[0].value, None);
    assert_eq!(coords.members[1].name, "Y");
    assert!(coords.members[1].value.is_some());
}

#[test]
fn parses_struct_instantiation_via_new_without_array_brackets() {
    let script = parse_with_mode(
        r#"ScriptName StructUsage

Struct Coordinates
    Float X
    Float Y
EndStruct

Function Test()
    Coordinates local = new Coordinates
EndFunction
"#,
        GameEdition::Fallout4,
    )
    .expect("struct instantiation should parse in Fallout 4 mode");

    let Some(papyrus_parser::ast::Stmt::VarDecl(decl)) = script.functions[0].body.first() else {
        panic!("expected a variable declaration");
    };
    assert_eq!(
        decl.value,
        Some(Expr::NewStruct {
            type_name: "Coordinates".to_string()
        })
    );
}

#[test]
fn new_with_brackets_still_creates_an_array_in_fallout4_mode() {
    let script = parse_with_mode(
        "ScriptName StillArrays\nFunction Test()\n    Int[] values = new Int[5]\nEndFunction\n",
        GameEdition::Fallout4,
    )
    .expect("array creation should still parse in Fallout 4 mode");

    let Some(papyrus_parser::ast::Stmt::VarDecl(decl)) = script.functions[0].body.first() else {
        panic!("expected a variable declaration");
    };
    assert!(matches!(decl.value, Some(Expr::NewArray { .. })));
}

#[test]
fn parses_a_property_group_with_collapse_flags() {
    let script = parse_with_mode(
        r#"ScriptName GroupedProperties

Group Settings CollapsedOnBase CollapsedOnRef
    Int Property MaxCount = 10 Auto
    Bool Property Enabled Auto
EndGroup
"#,
        GameEdition::Fallout4,
    )
    .expect("a Fallout 4 property group should parse");

    assert!(script.properties.is_empty());
    assert_eq!(script.groups.len(), 1);
    let group = &script.groups[0];
    assert_eq!(group.name, "Settings");
    assert!(group.is_collapsed_on_base);
    assert!(group.is_collapsed_on_ref);
    assert_eq!(group.properties.len(), 2);
    assert_eq!(group.properties[0].name, "MaxCount");
    assert_eq!(group.properties[1].name, "Enabled");
}

#[test]
fn parses_debug_only_and_beta_only_function_flags() {
    let script = parse_with_mode(
        "ScriptName FlaggedFunctions\n\nFunction LogDebug() DebugOnly\nEndFunction\n\nFunction LogBeta() BetaOnly\nEndFunction\n",
        GameEdition::Fallout4,
    )
    .expect("DebugOnly/BetaOnly should parse in Fallout 4 mode");

    assert!(script.functions[0].is_debug_only);
    assert!(!script.functions[0].is_beta_only);
    assert!(script.functions[1].is_beta_only);
    assert!(!script.functions[1].is_debug_only);
}

#[test]
fn skyrim_mode_leaves_debug_only_and_beta_only_unset() {
    // Skyrim mode never consumes DebugOnly/BetaOnly as flags, so a script
    // that never uses them (the common case) parses identically either
    // way, with both flags false.
    let script = parse("ScriptName Plain\n\nFunction Ordinary()\nEndFunction\n").unwrap();
    assert!(!script.functions[0].is_debug_only);
    assert!(!script.functions[0].is_beta_only);
    assert!(script.structs.is_empty());
    assert!(script.groups.is_empty());
}

#[test]
fn skyrim_mode_rejects_struct_declarations() {
    let error = parse("ScriptName Rejected\n\nStruct Coordinates\n    Float X\nEndStruct\n")
        .expect_err("Struct is Fallout 4 only");
    assert!(matches!(error, PapyrusError::Parse(_)));
}

#[test]
fn skyrim_mode_rejects_group_declarations() {
    let error = parse(
        "ScriptName Rejected\n\nGroup Settings\n    Int Property MaxCount = 10 Auto\nEndGroup\n",
    )
    .expect_err("Group is Fallout 4 only");
    assert!(matches!(error, PapyrusError::Parse(_)));
}

#[test]
fn skyrim_mode_rejects_debug_only_and_beta_only_flags() {
    let error = parse("ScriptName Rejected\n\nFunction LogDebug() DebugOnly\nEndFunction\n")
        .expect_err("DebugOnly is Fallout 4 only");
    assert!(matches!(error, PapyrusError::Parse(_)));
}

#[test]
fn skyrim_mode_rejects_bare_new_struct_instantiation() {
    // Without a `[size]`, `New <Name>` is only meaningful as Fallout 4's
    // struct instantiation; Skyrim mode still expects array brackets.
    let error = parse("ScriptName Rejected\n\nFunction Test()\n    Coordinates c = new Coordinates\nEndFunction\n")
        .expect_err("bare `New` requires array brackets outside Fallout 4 mode");
    assert!(matches!(error, PapyrusError::Parse(_)));
}

#[test]
fn group_rejects_a_non_property_member() {
    let error = parse_with_mode(
        "ScriptName Rejected\n\nGroup Settings\n    Function DoThing()\n    EndFunction\nEndGroup\n",
        GameEdition::Fallout4,
    )
    .expect_err("a Group may only contain Property declarations");
    assert!(matches!(error, PapyrusError::Parse(_)));
}
