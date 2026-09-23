//! Basic AST parser for Bethesda's Papyrus scripting language.
//!
//! This module is the foundation the lint rules build on: a lexer that
//! turns Papyrus source text into tokens, an AST describing a script's
//! structure, and a recursive-descent parser that builds the AST from the
//! token stream. [`parse`] and [`tokenize`] are both memoized against the
//! most recently seen source text (see [`cache`]): `papyrus-lints`'
//! `registry::collect_diagnostics` already calls each at most once per
//! `lint()`/`lint_with_external_arguments()` pass and hands the result to
//! every rule as a parameter rather than having each rule call back in
//! itself, but a `repair()` pass still calls into them once per fix applied
//! (each fix can change the source out from under any earlier result), all
//! against what's typically the exact same source text in a single repair
//! call.

pub mod ast;
mod cache;
pub mod comment_annotations;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod types;
pub mod visit;

use lexer::LexError;
use parser::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum PapyrusError {
    Lex(LexError),
    Parse(ParseError),
}

impl std::fmt::Display for PapyrusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PapyrusError::Lex(e) => write!(f, "{}:{}: {}", e.line, e.col, e.message),
            PapyrusError::Parse(e) => write!(f, "{}", e),
        }
    }
}

impl From<LexError> for PapyrusError {
    fn from(e: LexError) -> Self {
        PapyrusError::Lex(e)
    }
}

impl From<ParseError> for PapyrusError {
    fn from(e: ParseError) -> Self {
        PapyrusError::Parse(e)
    }
}

/// Parses Papyrus source text into a `Script` AST. Memoized against the
/// most recently seen `source` -- see [`cache`].
///
/// Always parses Skyrim's Papyrus dialect. See [`parse_with_mode`] to
/// parse Fallout 4's.
pub fn parse(source: &str) -> Result<ast::Script, PapyrusError> {
    cache::parse(source)
}

/// Same as [`parse`], but accepting `mode`'s Papyrus dialect (see
/// [`parser::GameEdition`]) rather than always parsing Skyrim's. Not
/// memoized: [`cache`]'s single slot is keyed by source text alone, and
/// caching it here too would return a stale result if the same source were
/// ever parsed under both modes.
pub fn parse_with_mode(
    source: &str,
    mode: parser::GameEdition,
) -> Result<ast::Script, PapyrusError> {
    let tokens = tokenize(source)?;
    Ok(parser::Parser::new_with_mode(tokens, mode).parse_script()?)
}

/// Lexes Papyrus source text into tokens, the same as
/// [`lexer::Lexer::new(source).tokenize()`](lexer::Lexer::tokenize).
/// Memoized against the most recently seen `source` -- see [`cache`] --
/// which is what lets every raw-token-based lint rule in `papyrus-lints`
/// call this directly instead of running its own lexer pass.
pub fn tokenize(source: &str) -> Result<Vec<token::Token>, LexError> {
    cache::tokenize(source)
}

/// Inserts a precomputed `ast` into [`parse`]'s in-memory memoization cache
/// as if `source` had just been parsed to it, so the next [`parse`] call
/// with the same `source` in this process returns it without re-parsing.
/// Lets a caller that already has a validated AST for `source` from
/// elsewhere (e.g. `papyrus-lint-core`'s disk-backed AST cache) short-circuit
/// this crate's own parse of it -- notably before calling into
/// `papyrus_lints::lint()`/`repair()`, which parse their `source` argument
/// internally without ever seeing this AST themselves.
pub fn prime_cache(source: &str, ast: ast::Script) {
    cache::prime(source, ast);
}

/// Same as [`prime_cache`], but for [`tokenize`]'s in-memory memoization
/// cache: inserts a precomputed `tokens` as if `source` had just been
/// lexed to it, so the next [`tokenize`] call with the same `source` in
/// this process returns it without re-lexing. Lets a caller that already
/// has a validated token stream for `source` from elsewhere (e.g.
/// `papyrus-lint-core`'s disk-backed AST cache) short-circuit this crate's
/// own lex of it -- notably before calling into
/// `papyrus_lints::lint()`/`repair()`, whose raw-token-based rules
/// tokenize their `source` argument internally without ever seeing these
/// tokens themselves.
///
/// # Panics
///
/// Panics when `tokens` is empty, because an empty stream cannot be passed
/// safely to [`parser::Parser`].
pub fn prime_tokenize_cache(source: &str, tokens: Vec<token::Token>) {
    cache::prime_tokens(source, tokens);
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
