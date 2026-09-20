//! The combined get-or-parse-and-cache operation built on top of
//! [`super::load`] and [`super::store`].

use std::path::Path;

use super::load::{get_in, get_tokens_in};
use super::store::{put_in, put_tokens_in};

/// Makes sure `papyrus_parser`'s in-memory memoization has both an AST and
/// a token stream ready for `source` before something that parses/
/// tokenizes `source` itself -- typically `papyrus_lints::lint()`/
/// `repair()`, called with only the raw source text, never `source_path` --
/// runs. A disk cache hit for either ([`get_in`]/[`get_tokens_in`]) already
/// primes the matching in-memory cache as a side effect; a miss for either
/// parses/tokenizes `source` once here instead (which populates the
/// in-memory cache the same way a hit would) and writes the result to the
/// disk cache for next time. See [`crate::ensure_primed`], the public
/// wrapper that supplies the real cache directory and version.
pub(crate) fn ensure_primed_in(
    dir: &Path,
    game: papyrus_parser::Game,
    source_path: &Path,
    source: &str,
    linter_version: &str,
) {
    if get_in(dir, game, source_path, source).is_none() {
        if let Ok(ast) = papyrus_parser::parse_for_game(game, source) {
            put_in(dir, game, source_path, source, &ast, linter_version);
        }
    }
    if get_tokens_in(dir, game, source_path, source).is_none() {
        if let Ok(tokens) = papyrus_parser::tokenize(source) {
            put_tokens_in(dir, game, source_path, source, &tokens, linter_version);
        }
    }
}

#[cfg(test)]
#[path = "prime_tests.rs"]
mod tests;
