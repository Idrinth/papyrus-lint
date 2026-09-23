//! In-memory memoization of [`crate::parse`] and [`crate::tokenize`].
//!
//! A single `repair()` pass over one script can call into these once per
//! fix applied, each against what's typically the exact same source text
//! (a fix that doesn't touch the source at all leaves it, and so this
//! cache's key, unchanged). This makes that free: a single-slot,
//! thread-local cache remembers only
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

/// Inserts a precomputed `ast` into this cache as if `source` had just
/// been parsed to it, so a subsequent [`parse`] call with the same
/// `source` in this process returns it directly instead of re-parsing.
/// Lets a caller that already has a validated AST for `source` from
/// elsewhere (e.g. `papyrus-lint-core`'s disk-backed `ast_cache`) prime
/// this cache before code that parses `source` itself -- without ever
/// seeing that AST -- runs, such as `papyrus_lints::lint()`.
pub(crate) fn prime(source: &str, ast: Script) {
    AST.with(|cell| {
        *cell.borrow_mut() = Some(Slot {
            source: source.to_string(),
            result: Ok(ast),
        });
    });
}

/// Same as [`prime`], but for [`tokenize`]'s cache slot: inserts a
/// precomputed `tokens` as if `source` had just been lexed to it, so a
/// subsequent [`tokenize`] call with the same `source` in this process
/// returns it directly instead of re-lexing. Lets a caller that already has
/// a validated token stream for `source` from elsewhere (e.g.
/// `papyrus-lint-core`'s disk-backed `ast_cache`) prime this cache before
/// code that tokenizes `source` itself -- without ever seeing those tokens
/// -- runs, such as a raw-token-based lint rule.
pub(crate) fn prime_tokens(source: &str, tokens: Vec<Token>) {
    assert!(
        !tokens.is_empty(),
        "cached parser token stream must not be empty"
    );
    TOKENS.with(|cell| {
        *cell.borrow_mut() = Some(Slot {
            source: source.to_string(),
            result: Ok(tokens),
        });
    });
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
