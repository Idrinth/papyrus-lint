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

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}

#[test]
fn does_not_flag_anything_without_a_resolver() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    MyMissingScript.StaticCall()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("Utility")
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        matches!(
            type_name.to_ascii_lowercase().as_str(),
            "int" | "bool" | "known" | "actor" | "objectreference"
        )
    }
}

#[test]
fn flags_a_call_through_a_script_that_cannot_be_located() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test()\n    Int a = MyMissingScript.StaticCall()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Script 'MyMissingScript' could not be located"));
}

#[test]
fn does_not_flag_a_known_native_singleton_script() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test()\n    Utility.Wait(1.0)\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unresolved_parent_and_declared_types_on_their_lines() {
    let diagnostics = check_with(
            "\nScriptName Example Extends MissingParent\n\nMissingProperty Property Value Auto\n\nMissingReturn Function Test(Known ok, MissingParam bad)\n    MissingLocal local\n    local = local as MissingCast\n    MissingArray[] values = new MissingElement[1]\nEndFunction\n",
            &mut FakeExternal,
        );

    let findings: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.line, diagnostic.message.as_str()))
        .collect();
    assert!(findings.contains(&(
        2,
        "[warning] Parent script 'MissingParent' could not be located"
    )));
    assert!(findings.contains(&(4, "[warning] Type 'MissingProperty' could not be located")));
    assert!(findings.contains(&(6, "[warning] Type 'MissingReturn' could not be located")));
    assert!(findings.contains(&(6, "[warning] Type 'MissingParam' could not be located")));
    assert!(findings.contains(&(7, "[warning] Type 'MissingLocal' could not be located")));
    assert!(findings.contains(&(8, "[warning] Type 'MissingCast' could not be located")));
    assert!(findings.contains(&(9, "[warning] Type 'MissingArray' could not be located")));
    assert!(findings.contains(&(9, "[warning] Type 'MissingElement' could not be located")));
    assert!(findings
        .iter()
        .all(|(_, message)| !message.contains("Known")));
}

#[test]
fn finds_missing_scripts_in_nested_control_flow_and_expressions() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test()
    Int[] values = new Int[MissingSize.Get()]
    values[MissingIndex.Get()] = MissingValue.Get()
    If MissingCondition.Get() && !MissingGuard.Get()
        MissingBody.Run(MissingArgument.Get())
    ElseIf MissingAlternative.Get()
        Return
    Else
        While MissingLoop.Get()
            values[0] = (MissingCast.Get() as Int)
        EndWhile
    EndIf
EndFunction
"#,
        &mut FakeExternal,
    );

    let mut missing_scripts: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            diagnostic
                .message
                .strip_prefix("[warning] Script '")
                .and_then(|message| message.strip_suffix("' could not be located"))
                .unwrap()
        })
        .collect();
    missing_scripts.sort_unstable();

    assert_eq!(
        missing_scripts,
        [
            "MissingAlternative",
            "MissingArgument",
            "MissingBody",
            "MissingCast",
            "MissingCondition",
            "MissingGuard",
            "MissingIndex",
            "MissingLoop",
            "MissingSize",
            "MissingValue",
        ]
    );
}

#[test]
fn finds_missing_scripts_in_return_values_and_state_functions() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Int Function GetValue()
    Return MissingReturn.Get()
EndFunction

State Active
    Function Run()
        MissingState.Run()
    EndFunction
EndState
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("MissingReturn"));
    assert!(diagnostics[1].message.contains("MissingState"));
}

#[test]
fn does_not_flag_a_call_through_a_local_variable_property_or_self() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Actor Property PlayerRef Auto

Function Test(ObjectReference akRef)
    akRef.SendAnimationEvent("Wave")
    PlayerRef.GetName()
    self.Test(None)
EndFunction
"#,
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn respects_locally_declared_names_case_insensitively() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Actor Property PlayerRef Auto

Function Test(ObjectReference TargetRef)
    playerref.GetName()
    TARGETREF.Activate(PlayerRef)
    Actor LocalRef = PlayerRef
    localref.GetName()
EndFunction
"#,
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_calls_in_script_level_initializers() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Int Property InitialValue = MissingPropertyValue.Get() AutoReadOnly
Int CurrentValue = MissingVariableValue.Get()
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("MissingPropertyValue"));
    assert_eq!(diagnostics[1].line, 5);
    assert!(diagnostics[1].message.contains("MissingVariableValue"));
}

#[test]
fn checks_both_sides_of_assignments_and_named_arguments() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test()
    MissingTarget.Get()[MissingIndex.Get()] = MissingValue.Get()
    Utility.Wait(afInterval = MissingArgument.Get())
EndFunction
"#,
        &mut FakeExternal,
    );

    let missing_scripts: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(
        diagnostics.iter().map(|d| d.line).collect::<Vec<_>>(),
        [5, 5, 5, 6]
    );
    assert!(missing_scripts
        .iter()
        .any(|message| message.contains("MissingTarget")));
    assert!(missing_scripts
        .iter()
        .any(|message| message.contains("MissingIndex")));
    assert!(missing_scripts
        .iter()
        .any(|message| message.contains("MissingValue")));
    assert!(missing_scripts
        .iter()
        .any(|message| message.contains("MissingArgument")));
    assert!(missing_scripts
        .iter()
        .all(|message| !message.contains("Utility")));
}

#[test]
fn reports_precise_metadata_for_an_unresolved_static_call() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test()\n    MissingScript.Run()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(
        diagnostics,
        [Diagnostic {
            line: 4,
            column: 22,
            message: "[warning] Script 'MissingScript' could not be located".to_string(),
            rule: RULE,
        }]
    );
}

#[test]
fn accepts_known_types_case_insensitively_in_every_declaration_position() {
    let diagnostics = check_with(
        r#"
ScriptName Example Extends ACTOR

KNOWN Property Value Auto

objectreference Function Test(aCtOr Subject)
    known LocalValue
    ObjectReference[] Values = new OBJECTREFERENCE[1]
    Return Subject
EndFunction
"#,
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_array_whose_element_type_resolves() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Actor[] Property Targets Auto

Actor[] Function Test(Actor[] actors)
    Actor[] local = actors
    ObjectReference[] casted = local as Actor[]
    Return casted
EndFunction
"#,
        &mut FakeExternal,
    );

    assert!(
        diagnostics.is_empty(),
        "array types should resolve through their element type, got {diagnostics:?}"
    );
}

#[test]
fn flags_an_unresolved_array_element_type_without_the_brackets() {
    let diagnostics = check_with(
        r#"
ScriptName Example

Function Test()
    Return 1 as MissingElement[]
EndFunction
"#,
        &mut FakeExternal,
    );

    assert_eq!(
        diagnostics,
        [Diagnostic {
            line: 5,
            column: 1,
            message: "[warning] Type 'MissingElement' could not be located".to_string(),
            rule: RULE,
        }]
    );
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

fn check_fallout4(source: &str, external: &mut impl ExternalSignatures) -> Vec<Diagnostic> {
    let ast =
        papyrus_parser::parse_with_mode(source, papyrus_parser::parser::GameEdition::Fallout4)
            .expect("Fallout 4 fixture should parse");
    super::check(
        source,
        Some(&ast),
        None,
        &crate::config::Config::default(),
        external,
    )
}

#[test]
fn does_not_flag_a_struct_declared_on_the_same_script() {
    let diagnostics = check_fallout4(
        r#"
ScriptName CompanionActorScript

EventData Property LastEvent Auto
EventData[] Property History Auto

EventData Function Test(eventdata data)
    EventData local
    Return data as EventData
EndFunction

Struct EventData
    Int Count
EndStruct
"#,
        &mut FakeExternal,
    );

    assert!(
        diagnostics.is_empty(),
        "a struct declared on this script is not a missing script, got {diagnostics:?}"
    );
}

#[test]
fn still_flags_a_missing_type_when_the_script_declares_other_structs() {
    let diagnostics = check_fallout4(
        r#"
ScriptName CompanionActorScript

EventData Property LastEvent Auto
MissingType Property Broken Auto

Struct EventData
    Int Count
EndStruct
"#,
        &mut FakeExternal,
    );

    assert_eq!(
        diagnostics,
        [Diagnostic {
            line: 5,
            column: 1,
            message: "[warning] Type 'MissingType' could not be located".to_string(),
            rule: RULE,
        }]
    );
}

/// `owner:struct` pairs this resolver treats as declared.
struct StructScope {
    /// Direct `Struct` declarations, `Script:Struct`.
    direct: &'static [(&'static str, &'static str)],
    /// `Extends` ancestry, including the named script, `Script:Struct`.
    ancestry: &'static [(&'static str, &'static str)],
}

impl ExternalSignatures for StructScope {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn script_exists(&mut self, _type_name: &str) -> bool {
        false
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        if matches!(
            type_name.to_ascii_lowercase().as_str(),
            "int" | "bool" | "string"
        ) {
            return true;
        }
        let known_script = |owner: &str| owner.eq_ignore_ascii_case(type_name);
        self.direct.iter().any(|(owner, _)| known_script(owner))
            || self.ancestry.iter().any(|(owner, _)| known_script(owner))
    }

    fn declares_struct(&mut self, type_name: &str, struct_name: &str) -> bool {
        self.direct.iter().any(|(owner, name)| {
            owner.eq_ignore_ascii_case(type_name) && name.eq_ignore_ascii_case(struct_name)
        })
    }

    fn declares_struct_in_ancestry(&mut self, type_name: &str, struct_name: &str) -> bool {
        self.ancestry.iter().any(|(owner, name)| {
            owner.eq_ignore_ascii_case(type_name) && name.eq_ignore_ascii_case(struct_name)
        })
    }
}

#[test]
fn does_not_flag_a_struct_inherited_or_imported() {
    let diagnostics = check_fallout4(
        r#"
ScriptName Example Extends WorkshopScript
Import FollowersScript

DailyUpdateData Property Update Auto
AffinityEventData Property Affinity Auto
MissingType Property Broken Auto

Function Test()
    MissingScript.Run()
EndFunction
"#,
        &mut StructScope {
            direct: &[("FollowersScript", "AffinityEventData")],
            ancestry: &[("WorkshopScript", "DailyUpdateData")],
        },
    );

    assert_eq!(diagnostics.len(), 2, "got {diagnostics:?}");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("MissingType")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("MissingScript")));
    assert!(diagnostics.iter().all(|diagnostic| {
        !diagnostic.message.contains("DailyUpdateData")
            && !diagnostic.message.contains("AffinityEventData")
    }));
}

#[test]
fn does_not_treat_an_imported_parent_struct_as_in_scope() {
    let diagnostics = check_fallout4(
        r#"
ScriptName Example
Import Other

DailyUpdateData Property Update Auto
"#,
        &mut StructScope {
            direct: &[],
            ancestry: &[("Base", "DailyUpdateData")],
        },
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("DailyUpdateData"));
}

#[test]
fn flags_a_missing_namespaced_script() {
    let diagnostics = check_fallout4(
        r#"
ScriptName Example Extends gamejam:GJDialogueScript
Import gamejam:GJDialogueScript

gamejam:GJDialogueScript Property Speaker Auto
gamejam:GJDialogueScript[] Property Speakers Auto

Function Test()
    gamejam:GJDialogueScript local
    local = local as gamejam:GJDialogueScript
    gamejam:GJDialogueScript.Run()
EndFunction
"#,
        &mut FakeExternal,
    );

    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Parent script 'gamejam:GJDialogueScript'")),
        "missing Extends parent should be flagged, got {messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Script 'gamejam:GJDialogueScript'")),
        "missing static call should be flagged, got {messages:?}"
    );
    assert!(
        messages
            .iter()
            .filter(|message| message.contains("Type 'gamejam:GJDialogueScript'"))
            .count()
            >= 4,
        "property, array, local, and cast should be flagged, got {messages:?}"
    );
}

#[test]
fn does_not_flag_a_namespaced_script_that_can_be_located() {
    let diagnostics = check_fallout4(
        r#"
ScriptName Example Extends gamejam:GJDialogueScript
Import gamejam:GJDialogueScript

gamejam:GJDialogueScript Property Speaker Auto
gamejam:GJDialogueScript[] Property Speakers Auto

Function Test()
    gamejam:GJDialogueScript local
    local = local as gamejam:GJDialogueScript
    gamejam:GJDialogueScript.Run()
EndFunction
"#,
        &mut LocatedNamespaced,
    );

    assert!(
        diagnostics.is_empty(),
        "a located namespaced script is not unresolved, got {diagnostics:?}"
    );
}

struct LocatedNamespaced;

impl ExternalSignatures for LocatedNamespaced {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("gamejam:GJDialogueScript")
    }

    fn type_exists(&mut self, type_name: &str) -> bool {
        let base = type_name.strip_suffix("[]").unwrap_or(type_name);
        base.eq_ignore_ascii_case("gamejam:GJDialogueScript")
    }
}

#[test]
fn still_flags_a_missing_struct_on_a_known_script() {
    let diagnostics = check_fallout4(
        r#"
ScriptName Example

Actor:MissingStruct Property Broken Auto
MissingType Property AlsoBroken Auto
"#,
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 2, "got {diagnostics:?}");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Actor:MissingStruct")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("MissingType")));
}
