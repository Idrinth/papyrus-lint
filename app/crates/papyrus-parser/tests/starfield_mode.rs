//! Starfield's Papyrus dialect is Fallout 4's plus header access flags
//! written as identifiers (`Private`, `Protected`, `SelfOnly`, `Internal`).
//! Those flags map onto the same [`AccessLevel`] values as `; @private` /
//! `; @protected`. Fallout 4 constructs that Starfield inherited are
//! covered in `fallout4_mode.rs`; this file is the Starfield-only delta.

use papyrus_parser::ast::{AccessLevel, LockKind, Stmt};
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
fn parses_requires_guard_on_variables_properties_and_functions() {
    let script = parse_with_mode(
        r#"ScriptName GuardedScript

Guard CoraGuardCount
int CoraStartingBookCount RequiresGuard(CoraGuardCount)
int property CurrentStateIndex = 0 Auto Hidden Conditional RequiresGuard(SetAnimationStateGuard)
RefCollectionAlias Property Alias_Passengers Mandatory RequiresGuard(PassengerGuard) Const Auto

Function Private_SetAnimationStateIndex(int newStateIndex, bool shouldUseJumpAnims=False) RequiresGuard(SetAnimationStateGuard) Private
EndFunction
"#,
        GameEdition::Starfield,
    )
    .expect("RequiresGuard should parse on Starfield declarations");

    assert_eq!(script.guards.len(), 1);
    assert_eq!(script.guards[0].name, "CoraGuardCount");
    assert_eq!(script.variables.len(), 1);
    assert_eq!(script.variables[0].name, "CoraStartingBookCount");
    assert_eq!(
        script.variables[0].requires_guard.as_deref(),
        Some("CoraGuardCount")
    );
    assert_eq!(
        script.properties[0].requires_guard.as_deref(),
        Some("SetAnimationStateGuard")
    );
    assert_eq!(
        script.properties[1].requires_guard.as_deref(),
        Some("PassengerGuard")
    );
    assert_eq!(
        script.functions[0].requires_guard.as_deref(),
        Some("SetAnimationStateGuard")
    );
    assert_eq!(script.functions[0].access_level, AccessLevel::Private);
}

#[test]
fn non_starfield_modes_reject_requires_guard() {
    let source = "ScriptName Rejected\nint Guarded RequiresGuard(MyGuard)\n";
    for mode in [GameEdition::Skyrim, GameEdition::Fallout4] {
        let error = parse_with_mode(source, mode)
            .expect_err("RequiresGuard is a Starfield-only declaration flag");
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

#[test]
fn parses_lock_guard_around_nested_statements() {
    let script = parse_with_mode(
        "ScriptName ATMScript\n\n\
         Function StealFromATM()\n\
             tempStealCount += 1\n\
             LockGuard stealGuard, auditGuard\n\
                 if GetState() == \"locked\"\n\
                     Return\n\
                 endif\n\
             EndLockGuard\n\
         EndFunction\n",
        GameEdition::Starfield,
    )
    .expect("LockGuard should parse in Starfield mode");

    let Stmt::LockGuard {
        kind,
        names,
        body,
        else_body,
        else_line,
        line,
        ..
    } = &script.functions[0].body[1]
    else {
        panic!(
            "expected a LockGuard statement, got {:?}",
            script.functions[0].body
        );
    };
    assert_eq!(*kind, LockKind::Lock);
    assert_eq!(names, &["stealGuard", "auditGuard"]);
    assert_eq!(*line, 5);
    assert!(else_body.is_empty());
    assert!(else_line.is_none());
    assert!(matches!(body[0], Stmt::If { .. }));
}

#[test]
fn parses_parenthesized_lock_guard_names() {
    let script = parse_with_mode(
        "ScriptName GuardScript\n\n\
         Function GuardedWork()\n\
             LockGuard(SpaceSceneGuard)\n\
             EndLockGuard\n\
             TryLockGuard(TaskMasterRestoreGuard)\n\
             EndTryLockGuard\n\
         EndFunction\n",
        GameEdition::Starfield,
    )
    .expect("parenthesized guard names should parse in Starfield mode");

    let body = &script.functions[0].body;
    let Stmt::LockGuard { kind, names, .. } = &body[0] else {
        panic!("expected a LockGuard, got {:?}", body[0]);
    };
    assert_eq!(*kind, LockKind::Lock);
    assert_eq!(names, &["SpaceSceneGuard"]);

    let Stmt::LockGuard { kind, names, .. } = &body[1] else {
        panic!("expected a TryLockGuard, got {:?}", body[1]);
    };
    assert_eq!(*kind, LockKind::Try);
    assert_eq!(names, &["TaskMasterRestoreGuard"]);
}

#[test]
fn parses_try_lock_guard_with_and_without_else() {
    let script = parse_with_mode(
        "ScriptName SQ_ParentScript\n\n\
         Function HandleCriticalHit()\n\
             TryLockGuard ShipCriticalHitGuard\n\
                 Debug.Trace(self)\n\
             ElseTryLockGuard\n\
                 Return\n\
             endTryLockGuard\n\
             trylockguard TaskMasterRestoreGuard, TaskMasterBackupGuard\n\
             EndTryLockGuard\n\
         EndFunction\n",
        GameEdition::Starfield,
    )
    .expect("TryLockGuard should parse in Starfield mode");

    let body = &script.functions[0].body;
    let Stmt::LockGuard {
        kind,
        names,
        body: locked,
        else_body,
        else_line,
        ..
    } = &body[0]
    else {
        panic!("expected a TryLockGuard, got {:?}", body[0]);
    };
    assert_eq!(*kind, LockKind::Try);
    assert_eq!(names, &["ShipCriticalHitGuard"]);
    assert!(matches!(locked[0], Stmt::Expr { .. }));
    assert!(matches!(else_body[0], Stmt::Return { .. }));
    assert_eq!(*else_line, Some(6));

    let Stmt::LockGuard {
        kind,
        names,
        body: locked,
        else_line,
        ..
    } = &body[1]
    else {
        panic!("expected a bare TryLockGuard, got {:?}", body[1]);
    };
    assert_eq!(*kind, LockKind::Try);
    assert_eq!(names, &["TaskMasterRestoreGuard", "TaskMasterBackupGuard"]);
    assert!(locked.is_empty());
    assert!(else_line.is_none());
}

#[test]
fn parses_guard_with_protects_function_logic() {
    let script = parse_with_mode(
        "ScriptName ATMScript\n\nGuard stealGuard ProtectsFunctionLogic\nint tempStealCount = 0\n",
        GameEdition::Starfield,
    )
    .expect("a flagged Guard should parse in Starfield mode");

    assert_eq!(script.guards.len(), 1);
    assert!(script
        .variables
        .iter()
        .all(|variable| variable.name != "stealGuard"));
    let guard = &script.guards[0];
    assert_eq!(guard.name, "stealGuard");
    assert!(guard.protects_function_logic);
    assert_eq!(script.variables.len(), 1);
    assert_eq!(script.variables[0].name, "tempStealCount");
}

#[test]
fn parses_bare_guards() {
    let script = parse_with_mode(
        "ScriptName COM_CoraBookGuard\n\nGuard CoraGuardCount\nGuard CoraGuardReward\n",
        GameEdition::Starfield,
    )
    .expect("bare Guards should parse in Starfield mode");

    assert_eq!(script.guards.len(), 2);
    assert!(script.variables.is_empty());
    assert_eq!(script.guards[0].name, "CoraGuardCount");
    assert!(!script.guards[0].protects_function_logic);
    assert_eq!(script.guards[1].name, "CoraGuardReward");
    assert!(!script.guards[1].protects_function_logic);
}
