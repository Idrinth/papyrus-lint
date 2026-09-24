use super::*;

fn kinds(source: &str) -> Vec<TokenKind> {
    Lexer::new(source)
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[test]
fn skips_whitespace_and_line_comments() {
    let toks = kinds("  ; a comment\nInt x");
    assert_eq!(
        toks,
        vec![
            TokenKind::Identifier("Int".to_string()),
            TokenKind::Identifier("x".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn ignores_a_leading_utf8_byte_order_mark() {
    let source = "ScriptName Example\n";
    let with_bom = format!("\u{feff}{source}");

    assert_eq!(kinds(&with_bom), kinds(source));

    let tokens = Lexer::new(&with_bom).tokenize().unwrap();
    assert_eq!((tokens[0].line, tokens[0].col), (1, 1));
}

#[test]
fn keeps_code_before_an_inline_comment_and_ignores_comment_text() {
    let toks = kinds("Actor a = Game.GetPlayer(); artificially slow GetValueInt()\nInt value = 1");
    assert_eq!(
        toks,
        vec![
            TokenKind::Identifier("Actor".into()),
            TokenKind::Identifier("a".into()),
            TokenKind::Assign,
            TokenKind::Identifier("Game".into()),
            TokenKind::Dot,
            TokenKind::Identifier("GetPlayer".into()),
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::Newline,
            TokenKind::Identifier("Int".into()),
            TokenKind::Identifier("value".into()),
            TokenKind::Assign,
            TokenKind::IntLiteral(1, IntFormat::Decimal),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn recognizes_block_and_brace_comments() {
    let toks = kinds("Int ;/ block \n comment /; x {doc} = 1");
    assert_eq!(
        toks,
        vec![
            TokenKind::Identifier("Int".to_string()),
            TokenKind::Identifier("x".to_string()),
            TokenKind::Assign,
            TokenKind::IntLiteral(1, IntFormat::Decimal),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn line_continuation_suppresses_newline() {
    let toks = kinds("Int x = 1 + \\\n2");
    assert_eq!(
        toks,
        vec![
            TokenKind::Identifier("Int".to_string()),
            TokenKind::Identifier("x".to_string()),
            TokenKind::Assign,
            TokenKind::IntLiteral(1, IntFormat::Decimal),
            TokenKind::Plus,
            TokenKind::IntLiteral(2, IntFormat::Decimal),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn keywords_are_case_insensitive() {
    let toks = kinds("SCRIPTNAME scriptName ScriptName");
    assert_eq!(
        toks,
        vec![
            TokenKind::Keyword(Keyword::ScriptName),
            TokenKind::Keyword(Keyword::ScriptName),
            TokenKind::Keyword(Keyword::ScriptName),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn reads_numbers_and_strings() {
    let toks = kinds("1 2.5 0x1F \"hi\\nthere\"");
    assert_eq!(
        toks,
        vec![
            TokenKind::IntLiteral(1, IntFormat::Decimal),
            TokenKind::FloatLiteral(2.5),
            TokenKind::IntLiteral(31, IntFormat::Hexadecimal),
            TokenKind::StringLiteral("hi\nthere".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn reads_operators() {
    let toks = kinds("== != >= <= && || += -= *= /= %=");
    assert_eq!(
        toks,
        vec![
            TokenKind::Eq,
            TokenKind::NotEq,
            TokenKind::GtEq,
            TokenKind::LtEq,
            TokenKind::AndAnd,
            TokenKind::OrOr,
            TokenKind::PlusAssign,
            TokenKind::MinusAssign,
            TokenKind::StarAssign,
            TokenKind::SlashAssign,
            TokenKind::PercentAssign,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn collapses_consecutive_newlines() {
    let toks = kinds("\n\n\nInt x\n\n\nInt y");
    assert_eq!(
        toks,
        vec![
            TokenKind::Identifier("Int".to_string()),
            TokenKind::Identifier("x".to_string()),
            TokenKind::Newline,
            TokenKind::Identifier("Int".to_string()),
            TokenKind::Identifier("y".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn reads_punctuation_single_character_operators_and_escapes() {
    let toks = kinds(r#"()[],.: + - * / % = ! > < "tab\tquote\"slash\\unknown\q""#);
    assert_eq!(
        toks,
        vec![
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Comma,
            TokenKind::Dot,
            TokenKind::Colon,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Assign,
            TokenKind::Not,
            TokenKind::Gt,
            TokenKind::Lt,
            TokenKind::StringLiteral("tab\tquote\"slash\\unknownq".to_string()),
            TokenKind::Eof,
        ]
    );
}

#[test]
fn supports_uppercase_hex_and_crlf_line_continuations() {
    let tokens = Lexer::new("0Xff \\\r\nname")
        .tokenize()
        .expect("valid tokens");
    assert_eq!(
        tokens[0].kind,
        TokenKind::IntLiteral(255, IntFormat::Hexadecimal)
    );
    assert_eq!(tokens[1].kind, TokenKind::Identifier("name".to_string()));
    assert_eq!((tokens[1].line, tokens[1].col), (2, 1));
}

#[test]
fn line_continuations_allow_horizontal_whitespace_before_newlines() {
    let tokens = Lexer::new("first \\ \t\n    second \\\t\r\nthird")
        .tokenize()
        .expect("valid continuations");

    assert_eq!(
        tokens
            .iter()
            .map(|token| (&token.kind, token.line, token.col))
            .collect::<Vec<_>>(),
        vec![
            (&TokenKind::Identifier("first".into()), 1, 1),
            (&TokenKind::Identifier("second".into()), 2, 5),
            (&TokenKind::Identifier("third".into()), 3, 1),
            (&TokenKind::Eof, 3, 6),
        ]
    );
}

#[test]
fn tracks_token_locations_across_crlf_and_comments() {
    let tokens = Lexer::new("one\r\n;/ two\r\nthree /; four\r\n{five\r\n}\tsix")
        .tokenize()
        .expect("valid tokens");

    let identifiers: Vec<_> = tokens
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::Identifier(name) => Some((name.as_str(), token.line, token.col)),
            _ => None,
        })
        .collect();
    assert_eq!(identifiers, [("one", 1, 1), ("four", 3, 10), ("six", 5, 3)]);
}

#[test]
fn emits_all_parser_relevant_annotations_from_one_comment() {
    let tokens = Lexer::new("Function Foo() ; @public @nodiscard @private")
        .tokenize()
        .expect("valid tokens");
    let annotations: Vec<_> = tokens
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::CommentAnnotation(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(annotations, ["public", "private"]);
}

#[test]
fn preserves_annotation_spelling_and_source_columns() {
    let tokens = Lexer::new("Int value ; prefix @Public, @PRIVATE")
        .tokenize()
        .expect("valid tokens");
    let annotations: Vec<_> = tokens
        .iter()
        .filter_map(|token| match &token.kind {
            TokenKind::CommentAnnotation(name) => Some((name.as_str(), token.line, token.col)),
            _ => None,
        })
        .collect();

    assert_eq!(annotations, [("Public", 1, 20), ("PRIVATE", 1, 29)]);
}

#[test]
fn reports_unterminated_comments_and_strings_at_their_start() {
    for (source, message, col) in [
        (";/ never closed", "unterminated block comment", 1),
        ("  {never closed", "unterminated comment block", 3),
        ("\"never closed", "unterminated string literal", 1),
        ("\"line\nbreak", "unterminated string literal", 1),
        ("\"trailing\\", "unterminated string literal", 1),
    ] {
        let error = Lexer::new(source).tokenize().unwrap_err();
        assert_eq!(error.message, message);
        assert_eq!((error.line, error.col), (1, col));
    }
}

#[test]
fn reports_invalid_characters_and_numeric_literals() {
    let unexpected = Lexer::new("Int x = @").tokenize().unwrap_err();
    assert_eq!(unexpected.message, "unexpected character '@'");
    assert_eq!((unexpected.line, unexpected.col), (1, 9));

    let invalid_hex = Lexer::new("0x").tokenize().unwrap_err();
    assert_eq!(invalid_hex.message, "invalid hex literal '0x'");

    let overflowing_integer = Lexer::new("999999999999999999999999")
        .tokenize()
        .unwrap_err();
    assert!(overflowing_integer
        .message
        .starts_with("invalid integer literal"));
}

#[test]
fn reports_lone_logical_operator_characters_at_their_locations() {
    for (source, character, line, col) in [("&", '&', 1, 1), ("one\n  |", '|', 2, 3)] {
        let error = Lexer::new(source).tokenize().unwrap_err();
        assert_eq!(error.message, format!("unexpected character '{character}'"));
        assert_eq!((error.line, error.col), (line, col));
    }
}
