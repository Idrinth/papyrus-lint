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
fn flags_backslash_n_in_trace_string() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Debug.Trace(\"Quest started\\n\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("\\n"));
    assert!(diagnostics[0].message.contains("no escape sequences"));
    // Indent(4) + Debug.Trace((12) + quote => backslash after Quest started at col 31.
    assert_eq!(diagnostics[0].column, 31);
}

#[test]
fn flags_backslash_t_and_backslash_r() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Debug.Notification(\"Done\\tOK\")\n    String cr = \"a\\rb\"\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("\\t"));
    assert!(diagnostics[1].message.contains("\\r"));
}

#[test]
fn flags_every_escape_in_the_same_string() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String s = \"a\\nb\\tc\\rd\"\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(diagnostics[0].column, 18);
    assert_eq!(diagnostics[1].column, 21);
    assert_eq!(diagnostics[2].column, 24);
}

#[test]
fn does_not_flag_strings_without_flagged_escapes() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String plain = \"hello\"\n    String path = \"C:\\\\Games\"\n    String other = \"C:\\Games\"\n    String quote = \"say \\\"hi\\\"\"\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_backslash_before_non_escape_letter() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String s = \"price\\x42\"\n    String f = \"form\\f\"\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_escapes_in_comments() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    ; Debug.Trace(\"ignore\\n\")\n    {/ \"doc\\t\" /}\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_windows_path_segment_that_looks_like_newline() {
    // "C:\new" contains `\n` — a classic footgun even in C, and exactly the
    // misconception this lint exists to catch in Papyrus.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String path = \"C:\\new\"\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("\\n"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\\nEndFunction\n").is_empty());
}

#[test]
fn does_not_crash_when_string_tokenization_fails() {
    // Unterminated string → tokenize fails → visitor gets no tokens.
    assert!(check("ScriptName Example\n\nFunction Test()\n    String s = \"unterminated\nEndFunction\n").is_empty());
}

#[test]
fn malformed_string_token_locations_are_ignored() {
    use papyrus_parser::token::{Token, TokenKind};

    let cases = [
        ("\"value\"", Token::new(TokenKind::StringLiteral("value".into()), 0, 1)),
        ("\"value\"", Token::new(TokenKind::StringLiteral("value".into()), 2, 1)),
        ("\"value\"", Token::new(TokenKind::StringLiteral("value".into()), 1, 0)),
        (
            "\n\"value\"",
            Token::new(TokenKind::StringLiteral("value".into()), 2, usize::MAX),
        ),
    ];

    for (source, token) in cases {
        let diagnostics = super::check(
            source,
            None,
            Some(&[token]),
            &crate::config::Config::default(),
            &mut crate::external_signatures::NoExternalSignatures,
        );

        assert!(diagnostics.is_empty());
    }
}

#[test]
fn raw_string_scan_stops_at_closing_quote_or_line_ending() {
    let mut escapes = Vec::new();

    visit_escapes_in_string("\"ok\"\\n", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });
    visit_escapes_in_string("\"ok\n\\t", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });
    visit_escapes_in_string("\"ok\r\\t", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });

    assert!(escapes.is_empty());
}

#[test]
fn raw_string_scan_ignores_invalid_offsets_and_trailing_backslashes() {
    let mut escapes = Vec::new();

    visit_escapes_in_string("not a string", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });
    visit_escapes_in_string("\"trailing\\", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });

    assert!(escapes.is_empty());
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test()\n    Debug.Trace(\"hi\\n\") ; @disable invalid-string-escape\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file invalid-string-escape\nScriptName Example\n\nFunction Test()\n    Debug.Trace(\"hi\\n\")\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let mut config = crate::config::Config::default();
    config.rules.invalid_string_escape = false;

    let diagnostics = crate::lint(
        "ScriptName Example\n\nFunction Test()\n    Debug.Trace(\"hi\\n\")\nEndFunction\n",
        &config,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}
