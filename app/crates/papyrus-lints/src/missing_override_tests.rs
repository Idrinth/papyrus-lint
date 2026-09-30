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

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("DoThing")
        {
            Some(Vec::new())
        } else {
            None
        }
    }
}

#[test]
fn without_external_never_flags_anything() {
    let diagnostics =
        check("ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_unannotated_override() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("'DoThing'"));
    assert!(diagnostics[0].message.contains("@override"));
}

#[test]
fn does_not_flag_when_header_has_override_annotation() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction DoThing() ; @override\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_line_above_has_override_annotation() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\n; @override\nFunction DoThing()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn annotation_is_case_insensitive() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction DoThing() ; @Override\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_overrideable_as_the_annotation() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction DoThing() ; @overrideable\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_function_with_no_matching_inherited_name() {
    let diagnostics = check_with(
        "ScriptName Example Extends ParentScript\n\nFunction SomethingElse()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_anything_on_a_script_without_extends() {
    let diagnostics = check_with(
        "ScriptName Example\n\nFunction DoThing()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_declared_only_inside_a_state() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function DoThing()\n    EndFunction\nEndState\n",
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

#[test]
fn flags_each_matching_top_level_declaration() {
    let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n\nEvent DoThing()\nEndEvent\n",
            &mut FakeExternal,
        );

    let locations: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.line, diagnostic.column))
        .collect();
    assert_eq!(locations, vec![(3, 1), (6, 1)]);
}
