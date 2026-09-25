//! Fallout 4's Papyrus dialect: custom `Struct`s, property `Group`s,
//! Fallout-specific declaration flags, colon-qualified names, the `is`
//! type-check operator, and remote / custom events
//! (`Event OtherScript.EventName(...)`). All are opt-in through
//! [`GameEdition::Fallout4`]. Rejection of Fallout 4 constructs in Skyrim
//! mode lives in `skyrim_mode.rs`; Starfield-only flags are rejected here
//! and accepted in `starfield_mode.rs`.

use papyrus_parser::ast::Expr;
use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse_with_mode, PapyrusError};

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
fn parses_struct_member_flags() {
    let script = parse_with_mode(
        r#"ScriptName FlaggedStruct

Struct Tutorial
    Int TimesDisplayed Hidden
    Float LastTimeDisplayed = 1.0 Conditional
    Bool IsEnabled = True const
EndStruct
"#,
        GameEdition::Fallout4,
    )
    .expect("Fallout 4 struct member flags should parse");

    let tutorial = &script.structs[0];
    assert_eq!(tutorial.members.len(), 3);
    assert_eq!(tutorial.members[0].name, "TimesDisplayed");
    assert_eq!(tutorial.members[0].value, None);
    assert_eq!(tutorial.members[1].name, "LastTimeDisplayed");
    assert!(tutorial.members[1].value.is_some());
    assert_eq!(tutorial.members[2].name, "IsEnabled");
    assert!(tutorial.members[2].value.is_some());
}

#[test]
fn parses_struct_member_named_parent() {
    let script = parse_with_mode(
        r#"ScriptName ObjectReference

Struct ConnectPoint
    string parent
    string name
    float roll
    float pitch
EndStruct
"#,
        GameEdition::Fallout4,
    )
    .expect("FO4 struct member named parent should parse");

    assert_eq!(script.structs.len(), 1);
    let connect = &script.structs[0];
    assert_eq!(connect.name, "ConnectPoint");
    assert_eq!(connect.members.len(), 4);
    assert_eq!(connect.members[0].name, "parent");
    assert_eq!(connect.members[0].type_name.name, "string");
    assert_eq!(connect.members[1].name, "name");
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
fn parses_a_property_group_with_short_collapsed_flag_case_insensitively() {
    let script = parse_with_mode(
        r#"ScriptName GroupedProperties

Group Advanced cOlLaPsEd
    Bool Property OnBegin = false Const Auto
    Bool Property OnEnd = true Const Auto
EndGroup
"#,
        GameEdition::Fallout4,
    )
    .expect("the short Fallout 4 Collapsed group flag should parse");

    assert_eq!(script.groups.len(), 1);
    let group = &script.groups[0];
    assert_eq!(group.name, "Advanced");
    assert!(group.is_collapsed_on_base);
    assert!(group.is_collapsed_on_ref);
    assert_eq!(group.properties.len(), 2);
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
fn parses_debug_only_and_beta_only_script_flags() {
    parse_with_mode(
        "ScriptName Debug Native DebugOnly Hidden\n",
        GameEdition::Fallout4,
    )
    .expect("DebugOnly should parse as a Fallout 4 script flag");

    parse_with_mode(
        "ScriptName Beta Hidden BetaOnly Conditional\n",
        GameEdition::Fallout4,
    )
    .expect("BetaOnly should parse as a Fallout 4 script flag");
}

#[test]
fn parses_const_default_and_mandatory_flags_case_insensitively() {
    let script = parse_with_mode(
        r#"ScriptName User:WorkshopBellNPCsScript extends ObjectReference default Const

Scene Property CoreScene Auto Const
Cell Property DLC03Nucleus Auto mandatory const

Function StartTimer()
    Int iFailSafeTimerID = 1 Const
    Int lowercase = 2 const
EndFunction
"#,
        GameEdition::Fallout4,
    )
    .expect("Fallout 4 declaration flags should parse regardless of case");

    assert_eq!(script.name, "User:WorkshopBellNPCsScript");
    assert_eq!(script.properties.len(), 2);
    assert!(script.properties.iter().all(|property| property.is_auto));
    assert_eq!(script.functions[0].body.len(), 2);
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

#[test]
fn parses_colon_qualified_names_in_fallout4_mode() {
    let script = parse_with_mode(
        r#"ScriptName DLC01:DLC01_TrackSystemTrap extends DLC01:DLC01_TrackSystemTrapBase

Import DLC03:DLC03CoA_DiaNucVictorySermonScript

Hardcore:HC_ManagerScript Property HC_Manager Auto
WorkshopParentScript:WorkshopObjective[] Property WorkshopObjectives Auto

InstanceData:Owner Function GetInstanceOwner(Hardcore:HC_ManagerScript manager)
    InstanceData:Owner inst = new InstanceData:Owner
    DLC01:DLC01_UnstableModActorScript:UnstableModData data = new DLC01:DLC01_UnstableModActorScript:UnstableModData
    DLC01:DLC01_TrackSystemTrack[] active = new DLC01:DLC01_TrackSystemTrack[0]
    CreationClub:RescuedDogScript named = inst as CreationClub:RescuedDogScript
    CreationClub:RescuedDogScript[] namedArray = active as CreationClub:RescuedDogScript[]
    DLC04:DLC04_RQ_ManagerScript.GetScript()
    return inst
EndFunction

Namespace:Helper Function Run()
EndFunction

Function Namespace:DoThing()
EndFunction
"#,
        GameEdition::Fallout4,
    )
    .expect("colon-qualified Fallout 4 names should parse");

    assert_eq!(script.name, "DLC01:DLC01_TrackSystemTrap");
    assert_eq!(
        script.extends.as_deref(),
        Some("DLC01:DLC01_TrackSystemTrapBase")
    );
    assert_eq!(
        script.imports[0].name,
        "DLC03:DLC03CoA_DiaNucVictorySermonScript"
    );
    assert_eq!(
        script.properties[0].type_name.name,
        "Hardcore:HC_ManagerScript"
    );
    assert!(!script.properties[0].type_name.is_array);
    assert_eq!(
        script.properties[1].type_name.name,
        "WorkshopParentScript:WorkshopObjective"
    );
    assert!(script.properties[1].type_name.is_array);

    let owner = &script.functions[0];
    assert_eq!(owner.name, "GetInstanceOwner");
    assert_eq!(
        owner.return_type.as_ref().map(|ty| ty.name.as_str()),
        Some("InstanceData:Owner")
    );
    assert_eq!(owner.params[0].type_name.name, "Hardcore:HC_ManagerScript");
    assert_eq!(owner.params[0].name, "manager");
    let Some(papyrus_parser::ast::Stmt::VarDecl(inst)) = owner.body.first() else {
        panic!("expected a namespaced local");
    };
    assert_eq!(inst.type_name.name, "InstanceData:Owner");
    assert_eq!(
        inst.value,
        Some(Expr::NewStruct {
            type_name: "InstanceData:Owner".to_string()
        })
    );
    let Some(papyrus_parser::ast::Stmt::VarDecl(data)) = owner.body.get(1) else {
        panic!("expected a nested struct local");
    };
    assert_eq!(
        data.value,
        Some(Expr::NewStruct {
            type_name: "DLC01:DLC01_UnstableModActorScript:UnstableModData".to_string()
        })
    );
    let Some(papyrus_parser::ast::Stmt::VarDecl(active)) = owner.body.get(2) else {
        panic!("expected a namespaced array local");
    };
    assert!(matches!(active.value, Some(Expr::NewArray { .. })));
    assert_eq!(active.type_name.name, "DLC01:DLC01_TrackSystemTrack");
    assert!(active.type_name.is_array);
    let Some(papyrus_parser::ast::Stmt::VarDecl(named)) = owner.body.get(3) else {
        panic!("expected a cast local");
    };
    assert_eq!(
        named.value,
        Some(Expr::Cast {
            value: Box::new(Expr::Identifier("inst".to_string())),
            type_name: "CreationClub:RescuedDogScript".to_string(),
        })
    );
    let Some(papyrus_parser::ast::Stmt::VarDecl(named_array)) = owner.body.get(4) else {
        panic!("expected a namespaced array cast local");
    };
    assert_eq!(
        named_array.value,
        Some(Expr::Cast {
            value: Box::new(Expr::Identifier("active".to_string())),
            type_name: "CreationClub:RescuedDogScript[]".to_string(),
        })
    );
    let Some(papyrus_parser::ast::Stmt::Expr { value, .. }) = owner.body.get(5) else {
        panic!("expected a namespaced call");
    };
    assert!(matches!(value, Expr::Call { .. }));
    assert_eq!(script.functions[1].name, "Run");
    assert_eq!(
        script.functions[1]
            .return_type
            .as_ref()
            .map(|ty| ty.name.as_str()),
        Some("Namespace:Helper")
    );
    assert_eq!(script.functions[2].name, "Namespace:DoThing");
}

#[test]
fn parses_remote_events_in_fallout4_mode() {
    let script = parse_with_mode(
        r#"ScriptName BoS301Script

Event WorkshopParentScript.WorkshopObjectBuilt(WorkshopParentScript akSender, Var[] akArgs)
EndEvent

Event Actor.OnLocationChange(Actor akSender, Location akOldLoc, Location akNewLoc)
EndEvent

Event ObjectReference.OnLoad(ObjectReference akSender)
EndEvent

Event RoachScareScript.flee(RoachScareScript akSender, Var[] akArgs)
EndEvent

Event DLC03:SomeQuest.OnStageSet(DLC03:SomeQuest akSender, Var[] akArgs)
EndEvent

State Active
    Event ObjectReference.OnActivate(ObjectReference akSender, ObjectReference akActionRef)
    EndEvent
EndState
"#,
        GameEdition::Fallout4,
    )
    .expect("Fallout 4 remote events should parse");

    assert_eq!(script.functions.len(), 5);
    assert!(script.functions.iter().all(|function| function.is_event));
    assert_eq!(
        script.functions[0].name,
        "WorkshopParentScript.WorkshopObjectBuilt"
    );
    assert_eq!(script.functions[0].params.len(), 2);
    assert_eq!(
        script.functions[0].params[0].type_name.name,
        "WorkshopParentScript"
    );
    assert_eq!(script.functions[0].params[0].name, "akSender");
    assert_eq!(script.functions[0].params[1].type_name.name, "Var");
    assert!(script.functions[0].params[1].type_name.is_array);
    assert_eq!(script.functions[1].name, "Actor.OnLocationChange");
    assert_eq!(script.functions[1].params.len(), 3);
    assert_eq!(script.functions[2].name, "ObjectReference.OnLoad");
    assert_eq!(script.functions[3].name, "RoachScareScript.flee");
    assert_eq!(script.functions[4].name, "DLC03:SomeQuest.OnStageSet");
    assert_eq!(
        script.functions[4].params[0].type_name.name,
        "DLC03:SomeQuest"
    );

    assert_eq!(script.states.len(), 1);
    assert_eq!(script.states[0].functions.len(), 1);
    assert!(script.states[0].functions[0].is_event);
    assert_eq!(
        script.states[0].functions[0].name,
        "ObjectReference.OnActivate"
    );
    assert_eq!(
        script.states[0].functions[0].state.as_deref(),
        Some("Active")
    );
}

#[test]
fn fallout4_mode_still_parses_plain_events() {
    let script = parse_with_mode(
        "ScriptName LocalEvents\n\nEvent OnInit()\nEndEvent\n",
        GameEdition::Fallout4,
    )
    .expect("plain events should still parse in Fallout 4 mode");
    assert!(script.functions[0].is_event);
    assert_eq!(script.functions[0].name, "OnInit");
}

#[test]
fn fallout4_mode_rejects_dotted_function_names() {
    let error = parse_with_mode(
        "ScriptName Rejected\n\nFunction Actor.DoThing()\nEndFunction\n",
        GameEdition::Fallout4,
    )
    .expect_err("dotted names are only valid on Event declarations");
    assert!(matches!(error, PapyrusError::Parse(_)));
}

#[test]
fn parses_is_type_check_operator() {
    let script = parse_with_mode(
        r#"ScriptName WorkshopSwitchIntervalScript

Event OnActivate(ObjectReference akActionRef)
    if akActionRef is Actor
        gotoState("Off")
    endif
EndEvent
"#,
        GameEdition::Fallout4,
    )
    .expect("Fallout 4 `is` type-check should parse");

    let Some(papyrus_parser::ast::Stmt::If { branches, .. }) = script.functions[0].body.first()
    else {
        panic!("expected an If statement");
    };
    assert_eq!(
        branches[0].condition,
        Expr::Is {
            value: Box::new(Expr::Identifier("akActionRef".to_string())),
            type_name: "Actor".to_string(),
        }
    );
}

#[test]
fn parses_is_with_colon_qualified_type() {
    let script = parse_with_mode(
        r#"ScriptName TypedCheck

Function Test(ObjectReference akRef)
    if akRef is DLC03:WorkshopNPCScript
        return
    endif
EndFunction
"#,
        GameEdition::Fallout4,
    )
    .expect("colon-qualified `is` type names should parse in Fallout 4 mode");

    let Some(papyrus_parser::ast::Stmt::If { branches, .. }) = script.functions[0].body.first()
    else {
        panic!("expected an If statement");
    };
    let Expr::Is { type_name, .. } = &branches[0].condition else {
        panic!("expected an `is` expression");
    };
    assert_eq!(type_name, "DLC03:WorkshopNPCScript");
}

#[test]
fn fallout4_mode_rejects_starfield_access_flags() {
    for flag in ["Private", "Protected", "SelfOnly", "Internal"] {
        let source = format!("ScriptName Rejected\nFunction Hide() {flag}\nEndFunction\n");
        let error = parse_with_mode(&source, GameEdition::Fallout4)
            .expect_err("Starfield header access flags are invalid in Fallout 4 mode");
        assert!(
            matches!(error, PapyrusError::Parse(_)),
            "{flag}: expected a parse error, got {error}"
        );
        assert!(
            error.to_string().contains("expected end of line"),
            "{flag}: Fallout 4 mode should stop at the header terminator, got {error}"
        );
    }
}
