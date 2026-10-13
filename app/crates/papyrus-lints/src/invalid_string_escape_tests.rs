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
fn flags_backslash_r_in_trace_string() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Debug.Trace(\"Quest started\\r\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'\\r'"));
    assert!(diagnostics[0].message.contains("not a supported escape"));
    // Indent(4) + Debug.Trace((12) + quote => backslash after Quest started at col 31.
    assert_eq!(diagnostics[0].column, 31);
}

#[test]
fn does_not_flag_compiler_supported_escapes() {
    // Issue #1761: the compiler translates \n, \t, \\ and \" (verified in
    // the .pex string table), so none of them may be reported.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Return \"AAA\\nBBB\\tCCC\\\\DDD\\\"EEE\"\n    Debug.Notification(\"Done\\tOK\\n\")\nEndFunction\n",
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn flags_only_unsupported_escapes_in_mixed_string() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String s = \"a\\nb\\rc\\td\\re\"\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(diagnostics[0].column, 21);
    assert_eq!(diagnostics[1].column, 27);
}

#[test]
fn escaped_backslash_before_r_is_not_flagged() {
    // `\\r` is an escaped backslash followed by a plain `r`.
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String s = \"C:\\\\readme\"\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
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
fn flags_windows_path_segment_that_looks_like_carriage_return() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    String path = \"C:\\readme\"\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'\\r'"));
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

    visit_escapes_in_string("\"ok\"\\r", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });
    visit_escapes_in_string("\"ok\n\\r", 1, 0, |column, letter| {
        escapes.push((column, letter));
    });
    visit_escapes_in_string("\"ok\r\\r", 1, 0, |column, letter| {
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
        "ScriptName Example\n\nFunction Test()\n    Debug.Trace(\"hi\\r\") ; @disable invalid-string-escape\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file invalid-string-escape\nScriptName Example\n\nFunction Test()\n    Debug.Trace(\"hi\\r\")\nEndFunction\n",
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
        "ScriptName Example\n\nFunction Test()\n    Debug.Trace(\"hi\\r\")\nEndFunction\n",
        &config,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}
