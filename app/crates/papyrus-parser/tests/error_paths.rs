use papyrus_parser::lexer::Lexer;
use papyrus_parser::parser::Parser;
use papyrus_parser::token::{Keyword, Token, TokenKind};
use papyrus_parser::{parse, PapyrusError};

#[test]
fn reports_missing_identifiers_in_declarations() {
    for (source, line, col, expected) in [
        ("ScriptName\n", 1, 11, "expected identifier, found Newline"),
        (
            "ScriptName Broken Extends\n",
            1,
            26,
            "expected identifier, found Newline",
        ),
        (
            "ScriptName Broken\nImport\n",
            2,
            7,
            "expected identifier, found Newline",
        ),
        (
            "ScriptName Broken\nInt Property Auto\n",
            2,
            14,
            "expected identifier, found Keyword(Auto)",
        ),
        (
            "ScriptName Broken\nFunction () Native\n",
            2,
            10,
            "expected identifier, found LParen",
        ),
        (
            "ScriptName Broken\nState\nEndState\n",
            2,
            6,
            "expected identifier, found Newline",
        ),
    ] {
        let PapyrusError::Parse(error) = parse(source).expect_err("declaration should be invalid")
        else {
            panic!("expected a parser error for {source:?}");
        };

        assert_eq!((error.line, error.col), (line, col));
        assert_eq!(error.message, expected);
    }
}

#[test]
fn reports_missing_declaration_punctuation_and_keywords() {
    for (source, expected) in [
        (
            "ScriptName Broken\nInt[ value\n",
            "expected RBracket, found Identifier(\"value\")",
        ),
        (
            "ScriptName Broken\nFunction Run Native\n",
            "expected LParen, found Keyword(Native)",
        ),
        (
            "ScriptName Broken\nFunction Run(Int) Native\n",
            "expected identifier, found RParen",
        ),
        (
            "ScriptName Broken\nFunction Run(Int value Native\n",
            "expected RParen, found Keyword(Native)",
        ),
        (
            "ScriptName Broken\nState Active\nInt Function Count() Native\nEndState extra\n",
            "expected end of line, found Identifier(\"extra\")",
        ),
    ] {
        let PapyrusError::Parse(error) = parse(source).expect_err("syntax should be invalid")
        else {
            panic!("expected a parser error for {source:?}");
        };

        assert_eq!(error.message, expected);
    }
}

#[test]
fn reports_incomplete_control_flow_and_expressions() {
    for (source, expected) in [
        (
            "ScriptName Broken\nFunction Run()\nIf true\nEndFunction\n",
            "unexpected token Keyword(EndFunction)",
        ),
        (
            "ScriptName Broken\nFunction Run()\nWhile true\nEndFunction\n",
            "unexpected token Keyword(EndFunction)",
        ),
        (
            "ScriptName Broken\nFunction Run()\nvalues[0\nEndFunction\n",
            "expected RBracket, found Newline",
        ),
        (
            "ScriptName Broken\nFunction Run()\nCall(1, )\nEndFunction\n",
            "unexpected token RParen",
        ),
        (
            "ScriptName Broken\nFunction Run()\nInt[] values = new Int[]\nEndFunction\n",
            "unexpected token RBracket",
        ),
        (
            "ScriptName Broken\nFunction Run()\nvalue as\nEndFunction\n",
            "expected identifier, found Newline",
        ),
    ] {
        let PapyrusError::Parse(error) = parse(source).expect_err("syntax should be incomplete")
        else {
            panic!("expected a parser error for {source:?}");
        };

        assert_eq!(error.message, expected);
    }
}

#[test]
fn parser_can_parse_a_standalone_expression_token_stream() {
    let tokens = Lexer::new("items[1].Length")
        .tokenize()
        .expect("expression should tokenize");
    let expression = Parser::new(tokens)
        .parse_expr()
        .expect("expression should parse");

    assert_eq!(
        format!("{expression:?}"),
        "Member { object: Index { object: Identifier(\"items\"), index: Literal(Int { value: 1, format: Decimal }) }, property: \"Length\" }"
    );
}

#[test]
#[should_panic(expected = "parser token stream must not be empty")]
fn parser_rejects_an_empty_token_stream() {
    Parser::new(Vec::new());
}

#[test]
fn parser_errors_use_the_current_token_location_and_display_format() {
    let mut parser = Parser::new(vec![
        Token::new(TokenKind::Keyword(Keyword::Return), 7, 13),
        Token::new(TokenKind::Eof, 7, 19),
    ]);
    let error = parser
        .parse_expr()
        .expect_err("Return cannot begin an expression");

    assert_eq!((error.line, error.col), (7, 13));
    assert_eq!(error.message, "unexpected token Keyword(Return)");
    assert_eq!(error.to_string(), "7:13: unexpected token Keyword(Return)");
}
