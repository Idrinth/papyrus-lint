use super::*;

#[test]
#[should_panic(expected = "cached parser token stream must not be empty")]
fn prime_tokens_rejects_an_empty_token_stream() {
    prime_tokens("ScriptName EmptyTokenCacheTest\n", Vec::new());
}

/// Two calls with the same content -- even from separately built
/// `String`s, so this isn't just pointer equality -- only lex once.
#[test]
fn tokenize_is_memoized_for_repeated_identical_source() {
    let before = TOKENIZE_COMPUTATIONS.with(|c| c.get());
    let source = format!("ScriptName {}\n", "TokenizeMemoTest");

    let first = tokenize(&source).unwrap();
    let second = tokenize(&source.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(TOKENIZE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn tokenize_recomputes_when_source_changes() {
    let before = TOKENIZE_COMPUTATIONS.with(|c| c.get());

    tokenize("ScriptName TokenizeChangeTestA\n").unwrap();
    tokenize("ScriptName TokenizeChangeTestB\n").unwrap();

    assert_eq!(TOKENIZE_COMPUTATIONS.with(|c| c.get()) - before, 2);
}

#[test]
fn tokenize_memoizes_lex_errors_too() {
    let before = TOKENIZE_COMPUTATIONS.with(|c| c.get());
    let source = "Int x = @TokenizeErrorMemoTest";

    let first = tokenize(source);
    let second = tokenize(source);

    assert!(first.is_err());
    assert_eq!(first, second);
    assert_eq!(TOKENIZE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn tokenize_only_retains_the_most_recent_source() {
    let before = TOKENIZE_COMPUTATIONS.with(|c| c.get());
    let first = "ScriptName TokenizeEvictionTestA\n";

    tokenize(first).unwrap();
    tokenize("ScriptName TokenizeEvictionTestB\n").unwrap();
    tokenize(first).unwrap();

    assert_eq!(TOKENIZE_COMPUTATIONS.with(|c| c.get()) - before, 3);
}

#[test]
fn parse_is_memoized_for_repeated_identical_source() {
    let before = PARSE_COMPUTATIONS.with(|c| c.get());
    let source = format!("ScriptName {}\n", "ParseMemoTest");

    let first = parse(&source).unwrap();
    let second = parse(&source.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn parse_recomputes_when_source_changes() {
    let before = PARSE_COMPUTATIONS.with(|c| c.get());

    parse("ScriptName ParseChangeTestA\n").unwrap();
    parse("ScriptName ParseChangeTestB\n").unwrap();

    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()) - before, 2);
}

#[test]
fn parse_memoizes_parser_errors_too() {
    let before = PARSE_COMPUTATIONS.with(|c| c.get());
    let source = "ScriptName ParseErrorMemoTest\nFunction Broken(\n";

    let first = parse(source);
    let second = parse(source);

    assert!(matches!(first, Err(PapyrusError::Parse(_))));
    assert_eq!(first, second);
    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn parse_memoizes_lexer_errors_too() {
    let before = PARSE_COMPUTATIONS.with(|c| c.get());
    let source = "ScriptName ParseLexErrorMemoTest\n@";

    let first = parse(source);
    let second = parse(source);

    assert!(matches!(first, Err(PapyrusError::Lex(_))));
    assert_eq!(first, second);
    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn parse_only_retains_the_most_recent_source() {
    let before = PARSE_COMPUTATIONS.with(|c| c.get());
    let first = "ScriptName ParseEvictionTestA\n";

    parse(first).unwrap();
    parse("ScriptName ParseEvictionTestB\n").unwrap();
    parse(first).unwrap();

    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()) - before, 3);
}

#[test]
fn prime_short_circuits_a_later_parse_of_the_same_source() {
    let before = PARSE_COMPUTATIONS.with(|c| c.get());
    let source = "ScriptName PrimeTest extends Quest\n";
    let ast = Parser::new(Lexer::new(source).tokenize().unwrap())
        .parse_script()
        .unwrap();

    prime(source, ast.clone());
    let result = parse(source).unwrap();

    assert_eq!(result, ast);
    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()), before);
}

#[test]
fn prime_is_overwritten_by_a_later_source_change() {
    let source = "ScriptName PrimeEvictionTest\n";
    let ast = Parser::new(Lexer::new(source).tokenize().unwrap())
        .parse_script()
        .unwrap();
    prime(source, ast);

    parse("ScriptName PrimeEvictionOther\n").unwrap();

    let before = PARSE_COMPUTATIONS.with(|c| c.get());
    parse(source).unwrap();
    assert_eq!(PARSE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn prime_tokens_short_circuits_a_later_tokenize_of_the_same_source() {
    let before = TOKENIZE_COMPUTATIONS.with(|c| c.get());
    let source = "ScriptName PrimeTokensTest extends Quest\n";
    let tokens = Lexer::new(source).tokenize().unwrap();

    prime_tokens(source, tokens.clone());
    let result = tokenize(source).unwrap();

    assert_eq!(result, tokens);
    assert_eq!(TOKENIZE_COMPUTATIONS.with(|c| c.get()), before);
}

#[test]
fn prime_tokens_is_overwritten_by_a_later_source_change() {
    let source = "ScriptName PrimeTokensEvictionTest\n";
    let tokens = Lexer::new(source).tokenize().unwrap();
    prime_tokens(source, tokens);

    tokenize("ScriptName PrimeTokensEvictionOther\n").unwrap();

    let before = TOKENIZE_COMPUTATIONS.with(|c| c.get());
    tokenize(source).unwrap();
    assert_eq!(TOKENIZE_COMPUTATIONS.with(|c| c.get()) - before, 1);
}

#[test]
fn parse_still_matches_a_fresh_lex_and_parse() {
    let source = "ScriptName ParseCorrectnessTest extends Quest\n\nInt Property MyValue = 1 Auto\n";

    let cached = parse(source).unwrap();
    let uncached = Parser::new(Lexer::new(source).tokenize().unwrap())
        .parse_script()
        .unwrap();

    assert_eq!(cached, uncached);
}
