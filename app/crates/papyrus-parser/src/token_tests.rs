use super::*;

#[test]
fn maps_every_keyword_spelling() {
    use Keyword::*;

    let cases = [
        ("scriptname", ScriptName),
        ("extends", Extends),
        ("hidden", Hidden),
        ("conditional", Conditional),
        ("import", Import),
        ("function", Function),
        ("endfunction", EndFunction),
        ("event", Event),
        ("endevent", EndEvent),
        ("property", Property),
        ("endproperty", EndProperty),
        ("auto", Auto),
        ("autoreadonly", AutoReadOnly),
        ("global", Global),
        ("native", Native),
        ("return", Return),
        ("if", If),
        ("elseif", ElseIf),
        ("else", Else),
        ("endif", EndIf),
        ("while", While),
        ("endwhile", EndWhile),
        ("state", State),
        ("endstate", EndState),
        ("new", New),
        ("as", As),
        ("is", Is),
        ("true", True),
        ("false", False),
        ("none", None),
        ("self", Self_),
        ("parent", Parent),
        ("length", Length),
        ("debugonly", DebugOnly),
        ("betaonly", BetaOnly),
        ("struct", Struct),
        ("endstruct", EndStruct),
        ("group", Group),
        ("endgroup", EndGroup),
        ("collapsed", Collapsed),
        ("collapsedonbase", CollapsedOnBase),
        ("collapsedonref", CollapsedOnRef),
    ];

    for (spelling, expected) in cases {
        assert_eq!(Keyword::from_word(spelling), Some(expected), "{spelling}");
    }
}

#[test]
fn rejects_non_keywords_and_non_normalized_case() {
    assert_eq!(Keyword::from_word("identifier"), None);
    assert_eq!(Keyword::from_word("customevent"), None);
    assert_eq!(Keyword::from_word("Function"), None);
    assert_eq!(Keyword::from_word(""), None);
}

#[test]
fn constructs_token_with_source_position() {
    let token = Token::new(TokenKind::Identifier("value".to_string()), 12, 7);

    assert_eq!(token.kind, TokenKind::Identifier("value".to_string()));
    assert_eq!(token.line, 12);
    assert_eq!(token.col, 7);
}

/// Tokens need to round-trip through serde so a disk-backed cache can
/// persist them alongside the parsed AST, rather than only ever holding
/// them in memory for the duration of a single lint pass.
#[test]
fn token_round_trips_through_json_including_a_keyword_and_hex_int_literal() {
    let tokens = vec![
        Token::new(TokenKind::Keyword(Keyword::ScriptName), 1, 1),
        Token::new(TokenKind::IntLiteral(42, IntFormat::Hexadecimal), 2, 5),
        Token::new(TokenKind::Eof, 3, 1),
    ];

    let json = serde_json::to_string(&tokens).unwrap();
    let round_tripped: Vec<Token> = serde_json::from_str(&json).unwrap();

    assert_eq!(round_tripped, tokens);
}
