use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_a_property_named_identically_to_its_script() {
    let diagnostics = check("ScriptName Example\n\nInt Property Example Auto\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("Property 'Example'"));
}

#[test]
fn flags_a_variable_named_identically_to_its_script() {
    let diagnostics = check("ScriptName Example\n\nInt Example = 1\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("Variable 'Example'"));
}

#[test]
fn matches_the_script_name_case_insensitively() {
    let diagnostics = check("ScriptName Example\n\nInt Property example Auto\n");

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_an_unrelated_property_or_variable_name() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyValue Auto\n\nInt total = 1\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_every_colliding_declaration_on_the_same_script() {
    let diagnostics = check("ScriptName Example\n\nInt Property Example Auto\n\nInt Example = 1\n");

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn does_not_flag_a_local_variable_sharing_the_script_name() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    Int Example = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nInt Property Example(\n").is_empty());
}
