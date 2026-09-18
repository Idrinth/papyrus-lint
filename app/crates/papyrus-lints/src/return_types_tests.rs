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
fn flags_mismatched_return_value() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return \"hi\"\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'Test'"));
    assert!(diagnostics[0].message.contains("declares return type Int"));
    assert!(diagnostics[0].message.contains("returns String"));
}

#[test]
fn allows_matching_return_value() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_int_returned_from_float_function() {
    let diagnostics =
        check("ScriptName Example\n\nFloat Function Test()\n    Return 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_functions_with_no_declared_return_type() {
    let diagnostics = check("ScriptName Example\n\nFunction Test()\n    Return\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_bare_return_in_a_typed_function() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test()\n    If true\n        Return\n    EndIf\n    Return 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_none_returned_from_a_primitive_function() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return None\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("returns None"));
}

#[test]
fn allows_none_returned_from_an_object_typed_function() {
    let diagnostics =
        check("ScriptName Example\n\nActor Function Test()\n    Return None\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_returns_nested_in_if_and_while_blocks() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return \"hi\"\n    Else\n        While flag\n            Return \"bye\"\n        EndWhile\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn does_not_flag_unresolvable_return_value() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Helper()\n    Return 1\nEndFunction\n\nInt Function Test()\n    Return Helper()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nInt Function Test(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

struct FakeExternalWithSubtypes;

impl ExternalSignatures for FakeExternalWithSubtypes {
    fn lookup(
        &mut self,
        _type_name: &str,
        _function_name: &str,
    ) -> Option<Vec<crate::external_signatures::ParamInfo>> {
        None
    }

    fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
        sub_type.eq_ignore_ascii_case("Armor") && super_type.eq_ignore_ascii_case("Form")
    }
}

#[test]
fn accepts_a_return_value_whose_script_extends_the_declared_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nForm Function Test()\n    Return MyArmor\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_an_unrelated_object_type() {
    let diagnostics = check_with(
            "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nForm Function Test()\n    Return MyWeapon\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("declares return type Form"));
    assert!(diagnostics[0].message.contains("returns Weapon"));
}
