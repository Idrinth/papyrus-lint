use super::*;

fn check_with(source: &str, mode: LineEndingsMode) -> Vec<Diagnostic> {
    let config = crate::config::Config {
        line_endings_mode: mode,
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
fn lf_accepts_lf() {
    assert!(check_with("ScriptName Example\nInt x = 1\n", LineEndingsMode::Lf).is_empty());
}

#[test]
fn lf_flags_crlf() {
    let diagnostics = check_with("ScriptName Example\r\nInt x = 1\r\n", LineEndingsMode::Lf);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 1);
    assert!(diagnostics[0].message.contains("lf"));
}

#[test]
fn crlf_flags_lf() {
    let diagnostics = check_with("ScriptName Example\nInt x = 1\n", LineEndingsMode::Crlf);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("crlf"));
}

#[test]
fn flags_mixed_endings_at_first_offender() {
    let diagnostics = check_with("ScriptName Example\nInt x = 1\r\n", LineEndingsMode::Lf);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 2);
}

#[test]
fn leaves_a_file_without_terminators_to_final_newline() {
    assert!(check_with("ScriptName Example", LineEndingsMode::Lf).is_empty());
    assert!(check_with("ScriptName Example", LineEndingsMode::Crlf).is_empty());
}

#[test]
fn rule_can_be_disabled_in_configuration() {
    let mut config = crate::config::Config::default();
    config.rules.line_endings = false;
    assert!(crate::lint("ScriptName Example\r\n", &config).is_empty());
}

#[test]
fn disable_file_comment_suppresses_the_diagnostic() {
    let source = "ScriptName Example\r\n; @disable-file line-endings\r\n";
    assert!(crate::lint(source, &crate::config::Config::default()).is_empty());
}

fn repair_with(source: &str, mode: LineEndingsMode) -> String {
    let config = crate::config::Config {
        line_endings_mode: mode,
        ..Default::default()
    };
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(source, ast.as_ref(), tokens.as_deref(), &config)
}

#[test]
fn repair_converts_crlf_to_lf() {
    assert_eq!(
        repair_with("ScriptName Example\r\nInt x = 1\r\n", LineEndingsMode::Lf),
        "ScriptName Example\nInt x = 1\n"
    );
}

#[test]
fn repair_converts_lf_to_crlf() {
    assert_eq!(
        repair_with("ScriptName Example\nInt x = 1\n", LineEndingsMode::Crlf),
        "ScriptName Example\r\nInt x = 1\r\n"
    );
}

#[test]
fn repair_does_not_invent_a_final_newline() {
    assert_eq!(
        repair_with("ScriptName Example", LineEndingsMode::Lf),
        "ScriptName Example"
    );
}

#[test]
fn repair_result_has_no_remaining_diagnostics() {
    let repaired = repair_with("ScriptName Example\r\nInt x = 1\n", LineEndingsMode::Lf);
    assert!(check_with(&repaired, LineEndingsMode::Lf).is_empty());
}
