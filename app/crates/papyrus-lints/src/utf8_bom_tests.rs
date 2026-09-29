use super::*;

fn check_with(source: &str, mode: Utf8BomMode) -> Vec<Diagnostic> {
    let config = crate::config::Config {
        utf8_bom_mode: mode,
        ..Default::default()
    };
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn forbidden_flags_a_bom() {
    let diagnostics = check_with("\u{FEFF}ScriptName Example\n", Utf8BomMode::Forbidden);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("UTF-8 BOM"));
}

#[test]
fn forbidden_accepts_no_bom() {
    assert!(check_with("ScriptName Example\n", Utf8BomMode::Forbidden).is_empty());
}

#[test]
fn required_flags_a_missing_bom() {
    let diagnostics = check_with("ScriptName Example\n", Utf8BomMode::Required);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn required_accepts_a_bom() {
    assert!(check_with("\u{FEFF}ScriptName Example\n", Utf8BomMode::Required).is_empty());
}

#[test]
fn allowed_never_flags() {
    assert!(check_with("ScriptName Example\n", Utf8BomMode::Allowed).is_empty());
    assert!(check_with("\u{FEFF}ScriptName Example\n", Utf8BomMode::Allowed).is_empty());
}

#[test]
fn rule_can_be_disabled_in_configuration() {
    let mut config = crate::config::Config::default();
    config.rules.utf8_bom = false;
    assert!(crate::lint("\u{FEFF}ScriptName Example\n", &config).is_empty());
}

#[test]
fn disable_file_comment_suppresses_the_diagnostic() {
    let source = "\u{FEFF}ScriptName Example\n; @disable-file utf8-bom\n";
    assert!(crate::lint(source, &crate::config::Config::default()).is_empty());
}

fn repair_with(source: &str, mode: Utf8BomMode) -> String {
    let config = crate::config::Config {
        utf8_bom_mode: mode,
        ..Default::default()
    };
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(source, ast.as_ref(), tokens.as_deref(), &config)
}

#[test]
fn repair_strips_bom_when_forbidden() {
    assert_eq!(
        repair_with("\u{FEFF}ScriptName Example\n", Utf8BomMode::Forbidden),
        "ScriptName Example\n"
    );
}

#[test]
fn repair_inserts_bom_when_required() {
    assert_eq!(
        repair_with("ScriptName Example\n", Utf8BomMode::Required),
        "\u{FEFF}ScriptName Example\n"
    );
}

#[test]
fn repair_result_has_no_remaining_diagnostics() {
    let stripped = repair_with("\u{FEFF}ScriptName Example\n", Utf8BomMode::Forbidden);
    assert!(check_with(&stripped, Utf8BomMode::Forbidden).is_empty());
    let added = repair_with("ScriptName Example\n", Utf8BomMode::Required);
    assert!(check_with(&added, Utf8BomMode::Required).is_empty());
}
