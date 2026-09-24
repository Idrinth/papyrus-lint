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

#[test]
fn flags_a_file_without_a_final_newline() {
    let diagnostics = check("ScriptName Example");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert_eq!(diagnostics[0].column, 19);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn reports_the_end_of_the_final_line() {
    let diagnostics = check("ScriptName Example\nString value = \"Héllo 🌍\"");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 2);
    assert_eq!(diagnostics[0].column, 25);
}

#[test]
fn accepts_lf_and_crlf_endings() {
    assert!(check("ScriptName Example\n").is_empty());
    assert!(check("ScriptName Example\r\n").is_empty());
}

#[test]
fn accepts_an_empty_file() {
    assert!(check("").is_empty());
}

#[test]
fn rule_can_be_disabled_in_configuration() {
    let mut config = crate::config::Config::default();
    config.rules.final_newline = false;

    assert!(crate::lint("ScriptName Example", &config).is_empty());
}

#[test]
fn disable_file_comment_suppresses_the_diagnostic() {
    let source = "ScriptName Example\n; @disable-file final-newline";

    assert!(crate::lint(source, &crate::config::Config::default()).is_empty());
}
