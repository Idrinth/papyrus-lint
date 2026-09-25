//! Starfield's Papyrus dialect is Fallout 4's plus header access flags
//! written as identifiers (`Private`, `Protected`, `SelfOnly`, `Internal`).
//! Those flags map onto the same [`AccessLevel`] values as `; @private` /
//! `; @protected`. Fallout 4 constructs that Starfield inherited are
//! covered in `fallout4_mode.rs`; this file is the Starfield-only delta
//! and the check that Fallout 4 mode still rejects those flags.

use papyrus_parser::ast::AccessLevel;
use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse_with_mode, PapyrusError};

#[test]
fn starfield_mode_parses_fallout4_structs() {
    let script = parse_with_mode(
        "ScriptName StructScript\n\nStruct Coordinates\n    Float X\nEndStruct\n",
        GameEdition::Starfield,
    )
    .expect("Starfield is a Fallout 4 dialect superset");
    assert_eq!(script.structs.len(), 1);
    assert_eq!(script.structs[0].name, "Coordinates");
}

#[test]
fn parses_private_and_protected_function_flags() {
    let script = parse_with_mode(
        r#"ScriptName CompanionCrimeResponseScript

Function ProcessCrimeFactionAnger(Actor ActorToTest) Private
EndFunction

Function CivilianKilled(Actor CivilianActor) Protected
EndFunction
"#,
        GameEdition::Starfield,
    )
    .expect("Starfield header access flags should parse");

    assert_eq!(script.functions[0].name, "ProcessCrimeFactionAnger");
    assert_eq!(script.functions[0].access_level, AccessLevel::Private);
    assert!(!script.functions[0].is_native);
    assert_eq!(script.functions[1].name, "CivilianKilled");
    assert_eq!(script.functions[1].access_level, AccessLevel::Protected);
}

#[test]
fn parses_native_protected_selfonly_in_any_order() {
    let script = parse_with_mode(
        r#"ScriptName ScriptObject

Function CancelTimer(int aiTimerID = 0) native protected selfonly
string Function GetState() protected selfonly
EndFunction
Function GotoState(string asNewState) SelfOnly Protected
EndFunction
Function HiddenNative() SelfOnly Native
"#,
        GameEdition::Starfield,
    )
    .expect("native protected selfonly should parse in Starfield mode");

    assert!(script.functions[0].is_native);
    assert_eq!(script.functions[0].access_level, AccessLevel::Protected);
    assert!(script.functions[0].body.is_empty());
    assert!(!script.functions[1].is_native);
    assert_eq!(script.functions[1].access_level, AccessLevel::Protected);
    assert_eq!(script.functions[1].name, "GetState");
    assert_eq!(script.functions[2].access_level, AccessLevel::Protected);
    assert!(script.functions[3].is_native);
    assert_eq!(script.functions[3].access_level, AccessLevel::Protected);
}

#[test]
fn selfonly_and_internal_fold_into_existing_access_levels() {
    let script = parse_with_mode(
        r#"ScriptName Flags

Function OnlySelf() SelfOnly
EndFunction

Function ScriptLocal() Internal
EndFunction

Function PublicDefault()
EndFunction
"#,
        GameEdition::Starfield,
    )
    .expect("SelfOnly/Internal should parse as AccessLevel");

    assert_eq!(script.functions[0].access_level, AccessLevel::Protected);
    assert_eq!(script.functions[1].access_level, AccessLevel::Private);
    assert_eq!(script.functions[2].access_level, AccessLevel::Public);
}

#[test]
fn header_flags_override_comment_annotations_when_both_appear() {
    let script = parse_with_mode(
        "ScriptName Mixed\nFunction Both() Private ; @protected\nEndFunction\n",
        GameEdition::Starfield,
    )
    .expect("mixed header flag and annotation should parse");
    // The identifier flag is consumed first; the trailing annotation then
    // overwrites it, matching `; @protected` as the last access token.
    assert_eq!(script.functions[0].access_level, AccessLevel::Protected);
}

#[test]
fn fallout4_and_skyrim_reject_starfield_access_flags() {
    let source = "ScriptName Rejected\nFunction Hide() Private\nEndFunction\n";
    for mode in [GameEdition::Skyrim, GameEdition::Fallout4] {
        let error =
            parse_with_mode(source, mode).expect_err("Private is a Starfield-only function flag");
        assert!(matches!(error, PapyrusError::Parse(_)), "{mode:?}: {error}");
        assert!(
            error.to_string().contains("expected end of line"),
            "{mode:?}: {error}"
        );
    }
}

#[test]
fn game_edition_helpers_describe_the_dialect_stack() {
    assert!(!GameEdition::Skyrim.has_fallout4_dialect());
    assert!(!GameEdition::Skyrim.has_starfield_dialect());
    assert!(GameEdition::Fallout4.has_fallout4_dialect());
    assert!(!GameEdition::Fallout4.has_starfield_dialect());
    assert!(GameEdition::Starfield.has_fallout4_dialect());
    assert!(GameEdition::Starfield.has_starfield_dialect());
}
