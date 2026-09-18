use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}
use crate::argument_types::ParamInfo;
use papyrus_parser::ast::TypeName;

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
        } else {
            None
        }
    }
}

#[test]
fn without_external_never_flags_anything() {
    let diagnostics = check(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_renamed_parameter_on_an_override() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Parameter 1 of 'DoThing'"));
    assert!(diagnostics[0].message.contains("named 'akRef'"));
    assert!(diagnostics[0].message.contains("names it 'akTarget'"));
}

#[test]
fn does_not_flag_parameter_names_that_only_differ_in_case() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference AKTARGET, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_each_mismatched_parameter_separately() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiTotal)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("Parameter 1"));
    assert!(diagnostics[1].message.contains("Parameter 2"));
}

#[test]
fn does_not_flag_a_function_with_no_matching_inherited_name() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction SomethingElse(Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_anything_on_a_script_without_extends() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_declared_only_inside_a_state() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function DoThing(ObjectReference akRef, Int aiCount)\n    EndFunction\nEndState\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_compare_parameters_beyond_the_shorter_declarations_count() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget)\nEndFunction\n",
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
