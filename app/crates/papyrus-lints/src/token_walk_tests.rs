//! Unit tests for the shared token-walking primitives.

use papyrus_parser::token::{Token, TokenKind};

use crate::token_walk::*;

fn lex(source: &str) -> Vec<Token> {
    papyrus_parser::tokenize(source).unwrap()
}

fn token_index(tokens: &[Token], predicate: impl Fn(&TokenKind) -> bool) -> usize {
    tokens
        .iter()
        .position(|token| predicate(&token.kind))
        .expect("expected token")
}

#[test]
fn line_starts_handles_empty_mixed_and_trailing_lines() {
    assert_eq!(line_starts(""), vec![0]);
    assert_eq!(line_starts("first\r\nsecond\n"), vec![0, 7, 14]);
}

#[test]
fn matching_parentheses_walk_in_both_directions_and_reject_unclosed_groups() {
    let tokens = lex("Call(One(1), 2)");
    let opens: Vec<_> = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| matches!(token.kind, TokenKind::LParen).then_some(index))
        .collect();
    let closes: Vec<_> = tokens
        .iter()
        .enumerate()
        .filter_map(|(index, token)| matches!(token.kind, TokenKind::RParen).then_some(index))
        .collect();

    assert_eq!(matching_close_paren(&tokens, opens[0]), Some(closes[1]));
    assert_eq!(matching_close_paren(&tokens, opens[1]), Some(closes[0]));
    assert_eq!(matching_open_paren(&tokens, closes[0]), Some(opens[1]));
    assert_eq!(matching_open_paren(&tokens, closes[1]), Some(opens[0]));

    let unclosed = lex("Call(One(1)");
    let outer_open = token_index(&unclosed, |kind| matches!(kind, TokenKind::LParen));
    assert_eq!(matching_close_paren(&unclosed, outer_open), None);

    let unopened = lex("One(1))");
    let last_close = unopened
        .iter()
        .rposition(|token| matches!(token.kind, TokenKind::RParen))
        .unwrap();
    assert_eq!(matching_open_paren(&unopened, last_close), None);
}

#[test]
fn identifier_and_qualifier_checks_are_case_insensitive_and_structural() {
    let tokens = lex("gAmE.GetFormFromFile(1, \"Test.esm\")");
    let call = token_index(
        &tokens,
        |kind| matches!(kind, TokenKind::Identifier(name) if name.eq_ignore_ascii_case("GetFormFromFile")),
    );

    assert!(is_identifier(&tokens[call], "getformfromfile"));
    assert!(qualifier_matches(&tokens, call, "game"));
    assert!(is_game_get_form_from_file_call(&tokens, call));
    assert!(!qualifier_matches(&tokens, 0, "game"));

    let unqualified = lex("GetFormFromFile(1, \"Test.esm\")");
    assert!(!is_game_get_form_from_file_call(&unqualified, 0));
    let not_a_call = lex("Game.GetFormFromFile");
    assert!(!is_game_get_form_from_file_call(&not_a_call, 2));
}

#[test]
fn split_arguments_ignores_nested_commas_and_handles_empty_calls() {
    let tokens = lex("Call(first, Nested(one, two), named = value)");
    let open = token_index(&tokens, |kind| matches!(kind, TokenKind::LParen));
    let close = matching_close_paren(&tokens, open).unwrap();
    let arguments = split_arguments(&tokens, open, close);

    assert_eq!(arguments.len(), 3);
    assert!(is_identifier(&tokens[arguments[0].0], "first"));
    assert!(is_identifier(&tokens[arguments[1].0], "Nested"));
    assert_eq!(
        skip_named_argument_prefix(&tokens, arguments[2].0, arguments[2].1),
        arguments[2].0 + 2
    );
    assert_eq!(
        skip_named_argument_prefix(&tokens, arguments[0].0, arguments[0].1),
        arguments[0].0
    );

    let empty = lex("Call()");
    let empty_open = token_index(&empty, |kind| matches!(kind, TokenKind::LParen));
    let empty_close = matching_close_paren(&empty, empty_open).unwrap();
    assert!(split_arguments(&empty, empty_open, empty_close).is_empty());
}

#[test]
fn top_level_operands_preserves_grouped_and_indexed_expressions() {
    let tokens = lex("left + (middle * nested) == values[index + 1] && !ready");
    let operands = top_level_operands(&tokens);

    assert_eq!(operands.len(), 4);
    assert!(is_identifier(&operands[0][0], "left"));
    assert!(matches!(operands[1][0].kind, TokenKind::LParen));
    assert!(is_identifier(&operands[2][0], "values"));
    assert!(is_identifier(&operands[3][0], "ready"));

    let leading = lex("!ready || || fallback");
    let operands = top_level_operands(&leading);
    assert_eq!(operands.len(), 2);
    assert!(is_identifier(&operands[0][0], "ready"));
    assert!(is_identifier(&operands[1][0], "fallback"));
    assert!(top_level_operands(&[]).is_empty());
}
