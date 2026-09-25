//! Skyrim's Papyrus dialect is the default: [`parse`] and
//! [`parse_with_mode`] with [`GameEdition::Skyrim`] must reject every
//! construct that [`GameEdition::Fallout4`] or [`GameEdition::Starfield`]
//! added. Positive coverage for those constructs lives in
//! `fallout4_mode.rs` and `starfield_mode.rs`.

use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse, parse_with_mode, PapyrusError};

fn assert_skyrim_rejects(source: &str, reason: &str) {
    let via_parse = parse(source).unwrap_err();
    assert!(
        matches!(via_parse, PapyrusError::Parse(_)),
        "{reason}: parse() should stay a parse error, got {via_parse}",
    );
    let via_mode = parse_with_mode(source, GameEdition::Skyrim).unwrap_err();
    assert!(
        matches!(via_mode, PapyrusError::Parse(_)),
        "{reason}: parse_with_mode(Skyrim) should stay a parse error, got {via_mode}",
    );
}

#[test]
fn parse_and_skyrim_mode_agree_on_a_plain_script() {
    let source = "ScriptName Plain\n\nFunction Ordinary()\nEndFunction\n";
    let via_parse = parse(source).expect("plain Skyrim script should parse");
    let via_mode = parse_with_mode(source, GameEdition::Skyrim)
        .expect("plain Skyrim script should parse in explicit Skyrim mode");

    assert_eq!(via_parse.name, via_mode.name);
    assert_eq!(via_parse.functions.len(), via_mode.functions.len());
    assert!(!via_parse.functions[0].is_debug_only);
    assert!(!via_parse.functions[0].is_beta_only);
    assert!(via_parse.structs.is_empty());
    assert!(via_parse.groups.is_empty());
    assert!(via_parse.custom_events.is_empty());
    assert!(via_mode.structs.is_empty());
    assert!(via_mode.groups.is_empty());
    assert!(via_mode.custom_events.is_empty());
}

#[test]
fn rejects_custom_event_declarations() {
    assert_skyrim_rejects(
        "ScriptName Example\nCustomEvent OnReady\n",
        "CustomEvent is a Fallout 4 and later declaration",
    );
}

#[test]
fn accepts_custom_event_as_an_identifier() {
    let script = parse(
        "ScriptName CustomEvent\n\n\
         Int customEvent = 1\n\n\
         Function CustomEvent()\nEndFunction\n",
    )
    .expect("Skyrim does not reserve CustomEvent");

    assert_eq!(script.name, "CustomEvent");
    assert_eq!(script.variables[0].name, "customEvent");
    assert_eq!(script.functions[0].name, "CustomEvent");
    assert!(script.custom_events.is_empty());
}

#[test]
fn accepts_declaration_flags_as_parameter_names() {
    let source = r#"ScriptName CKKeywordParam extends Quest

Int Function FlipConditionalState(Int conditional)
    Return conditional
EndFunction

Bool Function GetHiddenState(Bool hidden)
    Return hidden
EndFunction
"#;

    parse_with_mode(source, GameEdition::Skyrim)
        .expect("Skyrim permits Conditional and Hidden as parameter names");
}

#[test]
fn accepts_declaration_flags_as_named_argument_labels() {
    let source = r#"ScriptName CKKeywordNamedArgs

Function SetStates(Int conditional, Bool hidden)
EndFunction

Function UpdateStates()
    SetStates(conditional = 1, hidden = false)
EndFunction
"#;

    parse_with_mode(source, GameEdition::Skyrim)
        .expect("Skyrim permits Conditional and Hidden as named argument labels");
}

#[test]
fn rejects_struct_declarations() {
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nStruct Coordinates\n    Float X\nEndStruct\n",
        "Struct is Fallout 4 only",
    );
}

#[test]
fn rejects_group_declarations() {
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nGroup Settings\n    Int Property MaxCount = 10 Auto\nEndGroup\n",
        "Group is Fallout 4 only",
    );
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nGroup Settings CollapsedOnBase CollapsedOnRef\n    Int Property MaxCount = 10 Auto\nEndGroup\n",
        "Group collapse flags are Fallout 4 only",
    );
}

#[test]
fn rejects_bare_new_struct_instantiation() {
    // Without a `[size]`, `New <Name>` is only meaningful as Fallout 4's
    // struct instantiation; Skyrim mode still expects array brackets.
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nFunction Test()\n    Coordinates c = new Coordinates\nEndFunction\n",
        "bare `New` requires array brackets outside Fallout 4 mode",
    );
}

#[test]
fn rejects_debug_only_and_beta_only_flags() {
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nFunction LogDebug() DebugOnly\nEndFunction\n",
        "DebugOnly is Fallout 4 only",
    );
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nFunction LogBeta() BetaOnly\nEndFunction\n",
        "BetaOnly is Fallout 4 only",
    );
    for flag in ["DebugOnly", "BetaOnly"] {
        assert_skyrim_rejects(
            &format!("ScriptName Rejected {flag}\n"),
            "Fallout 4 script flags must remain invalid in Skyrim mode",
        );
    }
}

#[test]
fn rejects_fallout_4_declaration_flags() {
    for source in [
        "ScriptName Example Const\n",
        "ScriptName Example default\n",
        "ScriptName Example\nInt Property Value Auto Mandatory\n",
        "ScriptName Example\nFunction Test()\nInt value = 1 Const\nEndFunction\n",
    ] {
        assert_skyrim_rejects(source, "Fallout 4 flags must remain invalid in Skyrim mode");
    }

    let script = parse("ScriptName Const\nInt Mandatory = 1\nInt Property Default Auto\n")
        .expect("Fallout 4 flag spellings remain ordinary identifiers in Skyrim mode");
    assert_eq!(script.name, "Const");
    assert_eq!(script.variables[0].name, "Mandatory");
    assert_eq!(script.properties[0].name, "Default");
}

#[test]
fn rejects_colon_qualified_names() {
    for (source, reason) in [
        (
            "ScriptName Foo:Bar\n",
            "colon-qualified script names are Fallout 4 only",
        ),
        (
            "ScriptName Plain extends Foo:Bar\n",
            "colon-qualified extends is Fallout 4 only",
        ),
        (
            "ScriptName Plain\nImport Foo:Bar\n",
            "colon-qualified imports are Fallout 4 only",
        ),
        (
            "ScriptName Plain\nFoo:Bar Property X Auto\n",
            "colon-qualified property types are Fallout 4 only",
        ),
        (
            "ScriptName Plain\nFoo:Bar Function Run()\nEndFunction\n",
            "colon-qualified return types are Fallout 4 only",
        ),
        (
            "ScriptName Plain\nFunction Namespace:DoThing()\nEndFunction\n",
            "colon-qualified function names are Fallout 4 only",
        ),
        (
            "ScriptName Plain\nFunction Test()\n    Foo:Bar local = None\nEndFunction\n",
            "colon-qualified local types are Fallout 4 only",
        ),
        (
            "ScriptName Plain\nFunction Test()\n    Foo:Bar inst = new Foo:Bar\nEndFunction\n",
            "colon-qualified `new` is Fallout 4 only",
        ),
        (
            "ScriptName Plain\nFunction Test(ObjectReference akRef)\n    Foo:Bar named = akRef as Foo:Bar\nEndFunction\n",
            "colon-qualified casts are Fallout 4 only",
        ),
    ] {
        assert_skyrim_rejects(source, reason);
    }
}

#[test]
fn rejects_remote_events() {
    let error = parse(
        "ScriptName Rejected\n\nEvent Actor.OnLocationChange(Actor akSender, Location akOldLoc, Location akNewLoc)\nEndEvent\n",
    )
    .expect_err("remote events are Fallout 4 only");
    assert!(matches!(error, PapyrusError::Parse(_)));
    assert!(
        error.to_string().contains("expected LParen, found Dot"),
        "Skyrim mode should keep rejecting the `.` after an event name, got {error}",
    );

    assert_skyrim_rejects(
        "ScriptName Rejected\n\nEvent DLC03:SomeQuest.OnStageSet(DLC03:SomeQuest akSender, Var[] akArgs)\nEndEvent\n",
        "colon-qualified remote events are Fallout 4 only",
    );
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nState Active\n    Event ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)\n    EndEvent\nEndState\n",
        "remote events inside a state are Fallout 4 only",
    );
}

#[test]
fn rejects_is_type_check_operator() {
    let error = parse(
        r#"ScriptName Rejected

Event OnActivate(ObjectReference akActionRef)
    if akActionRef is Actor
        return
    endif
EndEvent
"#,
    )
    .expect_err("`is` is Fallout 4 only");
    assert!(matches!(error, PapyrusError::Parse(_)));
    assert!(
        error.to_string().contains("found Keyword(Is)")
            || error.to_string().contains("found Identifier(\"is\")"),
        "Skyrim mode should reject `is` as an unexpected token, got {error}",
    );

    assert_skyrim_rejects(
        r#"ScriptName Rejected

Function Test(ObjectReference akRef)
    if akRef is DLC03:WorkshopNPCScript
        return
    endif
EndFunction
"#,
        "colon-qualified `is` type names are Fallout 4 only",
    );
}

#[test]
fn rejects_starfield_access_flags() {
    for flag in ["Private", "Protected", "SelfOnly", "Internal"] {
        assert_skyrim_rejects(
            &format!("ScriptName Rejected\n\nFunction Hide() {flag}\nEndFunction\n"),
            &format!("{flag} is Starfield only"),
        );
    }
}

#[test]
fn rejects_lock_guard_statements() {
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nFunction Steal()\nLockGuard stealGuard\nEndLockGuard\nEndFunction\n",
        "LockGuard is Starfield only",
    );
    assert_skyrim_rejects(
        "ScriptName Rejected\n\nFunction Hit()\nTryLockGuard ShipCriticalHitGuard\nEndTryLockGuard\nEndFunction\n",
        "TryLockGuard is Starfield only",
    );
}
