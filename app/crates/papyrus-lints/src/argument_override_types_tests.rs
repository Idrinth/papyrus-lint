use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
    )
}

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}
use crate::argument_types::ParamInfo;

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(vec![
                ParamInfo {
                    name: "akTarget".to_string(),
                    type_name: TypeName {
                        name: "ObjectReference".to_string(),
                        is_array: false,
                    },
                },
                ParamInfo {
                    name: "aiCount".to_string(),
                    type_name: TypeName {
                        name: "Int".to_string(),
                        is_array: false,
                    },
                },
            ])
        } else if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("OnLoad")
        {
            Some(vec![ParamInfo {
                name: "abFirst".to_string(),
                type_name: TypeName {
                    name: "Bool".to_string(),
                    is_array: false,
                },
            }])
        } else {
            None
        }
    }
}

#[test]
fn without_external_never_flags_anything() {
    let diagnostics = check(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget, Int aiCount)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_a_matching_override() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_parameter_count_mismatch_on_an_override() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0]
        .message
        .contains("Function 'DoThing' declares 1 parameter"));
    assert!(diagnostics[0]
        .message
        .contains("inherited declaration on 'ParentScript' declares 2 parameters"));
}

#[test]
fn flags_a_parameter_type_mismatch_on_an_override() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget, String aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0]
        .message
        .contains("Parameter 2 of Function 'DoThing'"));
    assert!(diagnostics[0].message.contains("is declared String"));
    assert!(diagnostics[0].message.contains("declares Int"));
}

#[test]
fn flags_an_event_with_a_mismatched_parameter_type() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nEvent OnLoad(Int abFirst)\nEndEvent\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Parameter 1 of Event 'OnLoad'"));
    assert!(diagnostics[0].message.contains("is declared Int"));
    assert!(diagnostics[0].message.contains("declares Bool"));
}

#[test]
fn does_not_flag_a_parameter_type_that_only_differs_in_case() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(objectreference akTarget, int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_each_mismatched_parameter_type_separately() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(String akTarget, String aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("Parameter 1"));
    assert!(diagnostics[1].message.contains("Parameter 2"));
}

#[test]
fn does_not_check_parameter_types_when_the_count_already_mismatches() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(String akTarget)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("declares 1 parameter"));
}

#[test]
fn does_not_flag_a_function_with_no_matching_inherited_name() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction SomethingElse(String aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_anything_on_a_script_without_extends() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction DoThing(String akTarget, String aiCount)\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_declared_only_inside_a_state() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function DoThing(String akTarget, String aiCount)\n    EndFunction\nEndState\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}
