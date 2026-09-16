use super::*;
use crate::argument_types::ParamInfo;

#[test]
fn flags_local_variable_shadowing_own_property() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("own property"));
    assert!(diagnostics[0].message.contains("'MyValue'"));
}

#[test]
fn matches_property_shadowing_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int myvalue = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_local_variable_with_no_matching_property() {
    let diagnostics =
            check("ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int total = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_variable_declared_inside_if_block() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    If true\n        Int MyValue = 1\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn flags_variables_in_every_nested_control_flow_body() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    If true\n        While true\n            Int MyValue = 1\n        EndWhile\n    ElseIf false\n        Int myvalue = 2\n    Else\n        Int MYVALUE = 3\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![8, 11, 13]
    );
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nState Active\n    Function Test()\n        Int MyValue = 1\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'MyValue'"));
}

#[test]
fn does_not_flag_function_parameters_or_unrelated_properties() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test(Int MyValue)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_parent_shadowing_without_an_external_resolver() {
    let diagnostics = check(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

struct FakeExternalWithProperty;

impl ExternalSignatures for FakeExternalWithProperty {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn has_property(&mut self, type_name: &str, property_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("BaseScript")
            && property_name.eq_ignore_ascii_case("MyValue")
    }
}

#[test]
fn flags_local_variable_shadowing_a_parent_property_through_external_resolver() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
            &mut FakeExternalWithProperty,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("inherited from a parent script"));
}

#[test]
fn does_not_flag_unrelated_variable_through_external_resolver() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int total = 1\nEndFunction\n",
            &mut FakeExternalWithProperty,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_generated_local_shadowing_a_property_in_a_fragment_wrapper() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\nScriptname Example Extends TopicInfo Hidden\nFunction Fragment_0(ObjectReference akSpeakerRef)\nActor akSpeaker = akSpeakerRef as Actor\n;BEGIN CODE\nakSpeaker.RemoveItem(Gold001, 5)\n;END CODE\nEndFunction\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\nActor Property akSpeaker Auto\n";

    assert!(check(source).is_empty());
}

#[test]
fn own_property_takes_precedence_over_external_lookup() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
            &mut FakeExternalWithProperty,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("own property"));
}
