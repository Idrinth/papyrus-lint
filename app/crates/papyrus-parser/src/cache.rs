//! In-memory memoization of [`crate::parse`] and [`crate::tokenize`].
//!
//! A single lint pass over one script calls into these two dozens of times
//! -- one raw `Lexer::tokenize()` per token-based lint rule, one
//! `parse()` per AST-based one -- all against the exact same source text.
//! This makes that free: a single-slot, thread-local cache remembers only
//! the most recently seen source string (and the result computed for it),
//! so a repeat call with `source` unchanged is a string comparison and a
//! clone rather than a re-lex or re-parse.
//!
//! This is deliberately simpler than `papyrus-lint-core`'s disk-backed
//! `ast_cache`: that one exists to survive *across* separate desktop-app
//! commands and CLI invocations, so it has to key entries by file path and
//! validate them against the file's mtime, content hash, and the linter
//! version that wrote them. This cache never outlives the process (or even
//! the thread), so none of that bookkeeping applies -- the single slot is
//! simply overwritten whenever a different source string comes in.

use std::cell::RefCell;

use crate::ast::Script;
use crate::lexer::{LexError, Lexer};
use crate::parser::Parser;
use crate::token::Token;
use crate::PapyrusError;

struct Slot<T> {
    source: String,
    result: T,
}

type TokenizeResult = Result<Vec<Token>, LexError>;
type ParseResult = Result<Script, PapyrusError>;

thread_local! {
    static TOKENS: RefCell<Option<Slot<TokenizeResult>>> = const { RefCell::new(None) };
    static AST: RefCell<Option<Slot<ParseResult>>> = const { RefCell::new(None) };
}

// Counts actual (non-cached) computations, so this module's own tests can
// assert a repeat call was a cache hit rather than just checking the
// (identical either way) returned value.
#[cfg(test)]
thread_local! {
    static TOKENIZE_COMPUTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PARSE_COMPUTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Same as [`Lexer::new(source).tokenize()`](Lexer::tokenize), except
/// memoized against the most recently seen `source` (see the module docs).
pub(crate) fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let cached = TOKENS.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|slot| slot.source == source)
            .map(|slot| slot.result.clone())
    });
    if let Some(result) = cached {
        return result;
    }

    #[cfg(test)]
    TOKENIZE_COMPUTATIONS.with(|count| count.set(count.get() + 1));

    let result = Lexer::new(source).tokenize();
    TOKENS.with(|cell| {
        *cell.borrow_mut() = Some(Slot {
            source: source.to_string(),
            result: result.clone(),
        });
    });
    result
}

fn parse_uncached(source: &str) -> Result<Script, PapyrusError> {
    let tokens = tokenize(source)?;
    Ok(Parser::new(tokens).parse_script()?)
}

/// Same as [`crate::parse`], except memoized the same way as [`tokenize`]
/// (and reuses its cache for the lexing step underneath).
pub(crate) fn parse(source: &str) -> Result<Script, PapyrusError> {
    let cached = AST.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|slot| slot.source == source)
            .map(|slot| slot.result.clone())
    });
    if let Some(result) = cached {
        return result;
    }

    #[cfg(test)]
    PARSE_COMPUTATIONS.with(|count| count.set(count.get() + 1));

    let result = parse_uncached(source);
    AST.with(|cell| {
        *cell.borrow_mut() = Some(Slot {
            source: source.to_string(),
            result: result.clone(),
        });
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_still_matches_a_fresh_lex_and_parse() {
        let source =
            "ScriptName ParseCorrectnessTest extends Quest\n\nInt Property MyValue = 1 Auto\n";

        let cached = parse(source).unwrap();
        let uncached = Parser::new(Lexer::new(source).tokenize().unwrap())
            .parse_script()
            .unwrap();

        assert_eq!(cached, uncached);
    }
}
