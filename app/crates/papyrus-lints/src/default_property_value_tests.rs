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

#[test]
fn flags_auto_scalar_properties_with_no_default_value() {
    let source = "ScriptName Example\n\nBool Property IsActive Auto\nInt Property Count Auto\nFloat Property Scale Auto\nString Property Label Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 4);
    assert!(diagnostics.iter().all(|d| d.rule == RULE));
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("IsActive") && d.message.contains("False")));
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Count") && d.message.contains("0")));
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Scale") && d.message.contains("0.0")));
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Label") && d.message.contains("\"\"")));
}

#[test]
fn does_not_flag_properties_with_an_explicit_default_value() {
    let source =
        "ScriptName Example\n\nString Property abc = \"aBc\" Auto\nInt Property Count = 1 Auto\n";
    assert!(check(source).is_empty());
}

#[test]
fn flags_auto_read_only_properties_too() {
    let source = "ScriptName Example\n\nInt Property Count AutoReadOnly\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
}

#[test]
fn primitive_type_matching_is_case_insensitive() {
    let source = "ScriptName Example\n\nbool Property Ready Auto\niNT Property Count Auto\nfLoAt Property Scale Auto\nsTrInG Property Label Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 4);
    assert!(diagnostics[0].message.contains("False"));
    assert!(diagnostics[1].message.contains("'= 0'"));
    assert!(diagnostics[2].message.contains("0.0"));
    assert!(diagnostics[3].message.contains("\"\""));
}

#[test]
fn reports_each_declaration_at_its_own_line() {
    let source =
        "ScriptName Example\nBool Property Ready Auto\n\n\nString Property Label AutoReadOnly\n";
    let diagnostics = check(source);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.column))
            .collect::<Vec<_>>(),
        [(2, 1), (5, 1)]
    );
}

#[test]
fn explicit_false_zero_and_empty_string_defaults_are_not_flagged() {
    let source = "ScriptName Example\n\nBool Property Ready = False Auto\nInt Property Count = 0 AutoReadOnly\nFloat Property Scale = 0.0 Auto\nString Property Label = \"\" AutoReadOnly\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_object_typed_properties() {
    let source = "ScriptName Example\n\nActor Property PlayerRef Auto\n";
    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_array_typed_properties() {
    let source = "ScriptName Example\n\nInt[] Property Counts Auto\n";
    assert!(check(source).is_empty());
}

#[test]
fn does_not_flag_a_full_non_auto_property() {
    let source = "ScriptName Example\n\nInt Property Count\n\tInt Function Get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n";
    assert!(check(source).is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nInt Property Count = \"unterminated\n").is_empty());
}
