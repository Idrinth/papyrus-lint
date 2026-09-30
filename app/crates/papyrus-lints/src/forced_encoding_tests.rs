use super::*;

fn check_with(source: &str, encoding: EncodingEnforced) -> Vec<Diagnostic> {
    let config = crate::config::Config {
        encoding_enforced: encoding,
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
fn utf8_accepts_any_unicode() {
    assert!(check_with("ScriptName Example\n; \u{1F30D}\n", EncodingEnforced::Utf8).is_empty());
}

#[test]
fn latin1_flags_emoji() {
    let diagnostics = check_with("ScriptName Example\n; \u{1F30D}\n", EncodingEnforced::Iso88591);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("iso-8859-1"));
}

#[test]
fn windows_1252_accepts_euro() {
    assert!(check_with("ScriptName Example\n; \u{20AC}\n", EncodingEnforced::Windows1252).is_empty());
}

#[test]
fn windows_1252_flags_emoji() {
    let diagnostics = check_with("ScriptName Example\n; \u{1F30D}\n", EncodingEnforced::Windows1252);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn rule_can_be_disabled_in_configuration() {
    let mut config = crate::config::Config {
        encoding_enforced: EncodingEnforced::Iso88591,
        ..Default::default()
    };
    config.rules.forced_encoding = false;
    assert!(crate::lint("ScriptName Example\n; \u{1F30D}\n", &config).is_empty());
}

#[test]
fn disable_file_comment_suppresses_the_diagnostic() {
    let config = crate::config::Config {
        encoding_enforced: EncodingEnforced::Iso88591,
        ..Default::default()
    };
    let source = "ScriptName Example\n; \u{1F30D}\n; @disable-file forced-encoding\n";
    assert!(crate::lint(source, &config).is_empty());
}

fn repair_with(source: &str, encoding: EncodingEnforced) -> String {
    let config = crate::config::Config {
        encoding_enforced: encoding,
        ..Default::default()
    };
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(source, ast.as_ref(), tokens.as_deref(), &config)
}

#[test]
fn repair_replaces_unencodable_characters() {
    let repaired = repair_with("ScriptName Example\n; \u{1F30D}\n", EncodingEnforced::Iso88591);
    assert!(!repaired.contains('\u{1F30D}'));
    assert!(check_with(&repaired, EncodingEnforced::Iso88591).is_empty());
}

#[test]
fn repair_leaves_utf8_alone() {
    let source = "ScriptName Example\n; \u{1F30D}\n";
    assert_eq!(repair_with(source, EncodingEnforced::Utf8), source);
}
