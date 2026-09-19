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

fn repair(source: &str) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
    )
}

fn check_with<E: ExternalSignatures + ?Sized>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check_with(ast.as_ref(), external)
}
use crate::external_signatures::ParamInfo;

struct FakeExternal;

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn is_global_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        if type_name.eq_ignore_ascii_case("B") && function_name.eq_ignore_ascii_case("BC") {
            Some(true)
        } else {
            None
        }
    }

    fn can_resolve_script(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("B") || type_name.eq_ignore_ascii_case("D")
    }
}

#[test]
fn does_not_flag_anything_without_a_resolver() {
    let diagnostics =
        check("ScriptName A\n\nImport B\nImport D\n\nFunction C()\n    BC()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_import_whose_global_function_is_never_called_unqualified() {
    let diagnostics = check_with(
            "ScriptName A\n\nImport B ; provided BC, so loaded\nImport D ; should be mentioned as warning - unused import\n\nFunction C()\n    BC()\nEndFunction\n",
            &mut FakeExternal,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'D'"));
}

#[test]
fn does_not_flag_an_import_used_through_a_call_in_a_nested_branch() {
    let diagnostics = check_with(
            "ScriptName A\n\nImport B\n\nFunction C(Bool flag)\n    If flag\n        BC()\n    EndIf\nEndFunction\n",
            &mut FakeExternal,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_import_used_through_a_call_nested_in_an_argument() {
    let diagnostics = check_with(
        "ScriptName A\n\nImport B\n\nFunction C()\n    Debug.Trace(BC())\nEndFunction\n",
        &mut FakeExternal,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_a_qualified_call_to_the_same_function_name() {
    let diagnostics = check_with(
        "ScriptName A\n\nImport B\n\nFunction C()\n    Other.BC()\nEndFunction\n",
        &mut FakeExternal,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'B'"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName A\n\nImport B\n\nFunction C(\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_nothing_when_the_script_has_no_imports() {
    let diagnostics = check_with(
        "ScriptName A\n\nFunction C()\nEndFunction\n",
        &mut FakeExternal,
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn repair_removes_only_the_unused_imports_line() {
    let source = "ScriptName A\n\nImport B ; provided BC, so loaded\nImport D ; unused\n\nFunction C()\n    BC()\nEndFunction\n";

    let repaired = repair_with(source, &mut FakeExternal);

    assert_eq!(
            repaired,
            "ScriptName A\n\nImport B ; provided BC, so loaded\n\nFunction C()\n    BC()\nEndFunction\n"
        );
    assert!(check_with(&repaired, &mut FakeExternal).is_empty());
}

#[test]
fn repair_removes_multiple_unused_import_lines() {
    let source = "ScriptName A\n\nImport B\nImport D\n\nFunction C()\nEndFunction\n";

    let repaired = repair_with(source, &mut FakeExternal);

    assert_eq!(repaired, "ScriptName A\n\n\nFunction C()\nEndFunction\n");
}

#[test]
fn repair_leaves_a_used_imports_only_script_unchanged() {
    let source = "ScriptName A\n\nImport B\n\nFunction C()\n    BC()\nEndFunction\n";

    assert_eq!(repair_with(source, &mut FakeExternal), source);
}

#[test]
fn repair_without_a_resolver_never_removes_anything() {
    let source = "ScriptName A\n\nImport B\nImport D\n\nFunction C()\n    BC()\nEndFunction\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_preserves_crlf_line_endings() {
    let source = "ScriptName A\r\n\r\nImport D\r\n\r\nFunction C()\r\nEndFunction\r\n";

    let repaired = repair_with(source, &mut FakeExternal);

    assert_eq!(
        repaired,
        "ScriptName A\r\n\r\n\r\nFunction C()\r\nEndFunction\r\n"
    );
}

#[test]
fn repair_does_not_crash_on_unparseable_source() {
    let source = "ScriptName A\n\nImport B\n\nFunction C(\nEndFunction\n";

    assert_eq!(repair_with(source, &mut FakeExternal), source);
}
