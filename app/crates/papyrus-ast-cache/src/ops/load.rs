//! The read/lookup half of [`super`]: does a valid, still-fresh cache entry
//! exist for a given source, and if so, prime `papyrus_parser`'s in-memory
//! memoization with it.

use std::path::Path;

use crate::entry::valid_entry_in;

/// Also primes `papyrus_parser`'s own in-memory memoization (see
/// [`papyrus_parser::prime_cache`]) with a hit, so anything that parses
/// `source` itself later in this process -- notably
/// `papyrus_lints::lint()`/`repair()`, which never see `source_path` and so
/// can't consult this cache directly -- reuses it instead of re-parsing.
pub(crate) fn get_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
) -> Option<papyrus_parser::ast::Script> {
    let ast = valid_entry_in(dir, source_path, source)?.ast?;
    papyrus_parser::prime_cache(source, ast.clone());
    Some(ast)
}

/// Also primes `papyrus_parser`'s own in-memory memoization (see
/// [`papyrus_parser::prime_tokenize_cache`]) with a hit, the same way
/// [`get_in`] does for the AST.
pub(crate) fn get_tokens_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
) -> Option<Vec<papyrus_parser::token::Token>> {
    let tokens = valid_entry_in(dir, source_path, source)?.tokens?;
    papyrus_parser::prime_tokenize_cache(source, tokens.clone());
    Some(tokens)
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
