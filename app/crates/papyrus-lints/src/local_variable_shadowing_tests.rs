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

fn check_with<E: ExternalSignatures + ?Sized>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(source, ast.as_ref(), external)
}
use crate::external_signatures::ParamInfo;

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
fn flags_local_variable_shadowing_own_field() {
    let diagnostics =
        check("ScriptName Example\n\nInt MyValue = 1\n\nFunction Test()\n    Int MyValue = 2\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("own variable"));
    assert!(diagnostics[0].message.contains("'MyValue'"));
}

#[test]
fn matches_field_shadowing_case_insensitively() {
    let diagnostics =
        check("ScriptName Example\n\nInt MyValue = 1\n\nFunction Test()\n    Int myvalue = 2\nEndFunction\n");

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
fn check_with_returns_no_diagnostics_without_an_ast() {
    assert!(super::check_with(
        "ScriptName Example\n",
        None,
        &mut crate::external_signatures::NoExternalSignatures,
    )
    .is_empty());
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

#[test]
fn own_field_takes_precedence_over_external_lookup() {
    let diagnostics = check_with(
        "ScriptName Example Extends BaseScript\n\nInt MyValue = 0\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
        &mut FakeExternalWithProperty,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("own variable"));
}

#[test]
fn check_with_finds_declarations_in_nested_control_flow() {
    let diagnostics = check_with(
        "ScriptName Example\n\nInt MyValue = 0\n\nFunction Test()\n    If true\n        While true\n            Int MyValue = 1\n        EndWhile\n    ElseIf false\n        Int myvalue = 2\n    Else\n        Int MYVALUE = 3\n    EndIf\nEndFunction\n",
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![8, 11, 13]
    );
}

#[test]
fn flags_a_shadowing_local_inside_editable_fragment_code() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0()
;BEGIN CODE
Int MyValue = 1
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
Int Property MyValue Auto
";

    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn honors_a_disable_on_the_declaration_line() {
    let source = "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1 ; @disable local-variable-shadowing\nEndFunction\n";

    let diagnostics = crate::lint(source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_a_file_disable() {
    let source = "; @disable-file local-variable-shadowing\nScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n";

    let diagnostics = crate::lint(source, &crate::config::Config::default());

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_the_config_off_switch() {
    let source = "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.local_variable_shadowing = false;

    let diagnostics = crate::lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

struct FakeExternalWithField;

impl ExternalSignatures for FakeExternalWithField {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn has_field(&mut self, type_name: &str, field_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("BaseScript") && field_name.eq_ignore_ascii_case("MyField")
    }
}

#[test]
fn flags_local_variable_shadowing_a_parent_field_through_external_resolver() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyField = 1\nEndFunction\n",
            &mut FakeExternalWithField,
        );

    assert_eq!(diagnostics.len(), 1);
    // A parent field is reported as an inherited "variable", not a "property".
    assert!(diagnostics[0]
        .message
        .contains("variable 'MyField' inherited from a parent script"));
}

#[test]
fn does_not_flag_unrelated_variable_against_a_parent_field_resolver() {
    let diagnostics = check_with(
            "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int total = 1\nEndFunction\n",
            &mut FakeExternalWithField,
        );

    assert!(diagnostics.is_empty());
}
