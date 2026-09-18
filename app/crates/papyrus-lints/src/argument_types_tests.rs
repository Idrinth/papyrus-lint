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
fn flags_mismatched_literal_argument_to_local_function() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(1)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
    assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
    assert!(diagnostics[0].message.contains("expects String"));
    assert!(diagnostics[0].message.contains("got Int"));
}

#[test]
fn allows_int_argument_for_float_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction SetSpeed(Float speed)\nEndFunction\n\nFunction Test()\n    SetSpeed(1)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_float_argument_for_int_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction SetCount(Int count)\nEndFunction\n\nFunction Test()\n    SetCount(1.5)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects Int"));
    assert!(diagnostics[0].message.contains("got Float"));
}

#[test]
fn checks_variables_properties_and_casts_by_declared_type() {
    let diagnostics = check(
        r#"
ScriptName Example

Bool Property Enabled Auto

Function Configure(Bool flag)
EndFunction

Function Test()
    Int value = 5
    Configure(value)
    Configure(Enabled)
    Configure(value as Bool)
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 1 to 'Configure'"));
}

#[test]
fn allows_none_for_object_and_array_parameters_but_not_primitives() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Track(Actor akActor, Int[] values, Int count)
EndFunction

Function Test()
    Track(None, None, None)
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 3 to 'Track'"));
    assert!(diagnostics[0].message.contains("got None"));
}

#[test]
fn checks_self_qualified_and_unqualified_calls_the_same_way() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    self.Greet(1)\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn does_not_flag_a_nested_call_argument_since_its_return_type_is_unresolved() {
    // `infer_type` can't resolve what a call expression evaluates to
    // (see its docs), so an argument that is itself a call is always
    // skipped rather than checked — this only confirms that skip
    // doesn't crash or misfire.
    let diagnostics = check(
        r#"
ScriptName Example

Function Greet(String name)
EndFunction

String Function GetName()
    Return "hi"
EndFunction

Function Test()
    Greet(GetName())
EndFunction
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn skips_calls_with_unresolvable_target_or_argument_type() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Test(Actor akActor)
    akActor.SendAnimationEvent("Wave")
    Debug.Trace("hi")
    UnknownLocal()
EndFunction
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn skips_ambiguous_overrides_across_states() {
    let diagnostics = check(
        r#"
ScriptName Example

Function Greet(String name)
EndFunction

State Loud
    Function Greet(Int volume)
    EndFunction
EndState

Function Test()
    Greet(1)
EndFunction
"#,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_calls_beyond_the_declared_parameter_count() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\", 1)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_a_named_argument_against_its_matching_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA = 0, String argB = \"\")\nEndFunction\n\nEvent OnUpdate()\n    MyFunction(argB = 1)\nEndEvent\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Argument 2 to 'MyFunction'"));
    assert!(diagnostics[0].message.contains("expects String"));
    assert!(diagnostics[0].message.contains("got Int"));
}

#[test]
fn allows_a_correctly_typed_named_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA = 0, Int argB = 0)\nEndFunction\n\nEvent OnUpdate()\n    MyFunction(argB = 1)\nEndEvent\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn skips_an_unrecognized_named_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA = 0)\nEndFunction\n\nEvent OnUpdate()\n    MyFunction(argC = 1)\nEndEvent\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_a_named_argument_on_a_call_resolved_through_external_signatures() {
    // `FakeExternal` resolves full parameter info (including names) for
    // another script's function, so a named argument on such a call can
    // be matched to the parameter it fills just like a local one.
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(akTarget = 1)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 1 to 'MoveTo'"));
    assert!(diagnostics[0].message.contains("expects ObjectReference"));
    assert!(diagnostics[0].message.contains("got Int"));
}

#[test]
fn skips_an_unrecognized_named_argument_on_a_call_resolved_through_external_signatures() {
    let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(notAParam = 1)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if type_name.eq_ignore_ascii_case("Actor") && function_name.eq_ignore_ascii_case("MoveTo") {
            Some(vec![ParamInfo {
                name: "akTarget".to_string(),
                type_name: TypeName {
                    name: "ObjectReference".to_string(),
                    is_array: false,
                },
            }])
        } else {
            None
        }
    }
}

struct FakeExternalWithSubtypes;

impl ExternalSignatures for FakeExternalWithSubtypes {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ObjectReference")
            && function_name.eq_ignore_ascii_case("GetItemCount")
        {
            Some(vec![ParamInfo {
                name: "akItem".to_string(),
                type_name: TypeName {
                    name: "Form".to_string(),
                    is_array: false,
                },
            }])
        } else {
            None
        }
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case("Armor") && super_type.eq_ignore_ascii_case("Form")
    }
}

#[test]
fn accepts_an_argument_whose_script_extends_the_parameter_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test(ObjectReference akRef)\n    akRef.GetItemCount(MyArmor)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_an_unrelated_object_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nFunction Test(ObjectReference akRef)\n    akRef.GetItemCount(MyWeapon)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects Form"));
    assert!(diagnostics[0].message.contains("got Weapon"));
}

#[test]
fn check_with_resolves_calls_through_the_external_resolver() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(1)\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("expects ObjectReference"));
}
