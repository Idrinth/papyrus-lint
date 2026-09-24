//! The write/persistence half of [`super`]: stamping and writing a cache
//! entry, preserving whichever of the AST/tokens fields isn't being written
//! this call.

use std::path::Path;

use papyrus_lint_globals::Game;

use crate::entry::{
    file_modified_unix_secs, mtime_valid_entry_in_for_game, valid_entry_in_for_game,
    write_entry_in_for_game, CacheEntry,
};

enum FieldUpdate<'a> {
    Ast(&'a papyrus_parser::ast::Script),
    Tokens(&'a [papyrus_parser::token::Token]),
}

fn put_field_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    source: &str,
    update: FieldUpdate<'_>,
    linter_version: &str,
) {
    let existing = valid_entry_in_for_game(dir, game, source_path, source);
    let (ast, tokens) = match update {
        FieldUpdate::Ast(ast) => (Some(ast.clone()), existing.and_then(|entry| entry.tokens)),
        FieldUpdate::Tokens(tokens) => {
            (existing.and_then(|entry| entry.ast), Some(tokens.to_vec()))
        }
    };
    write_stamped_entry_for_game(dir, game, source_path, source, linter_version, ast, tokens);
}

pub(crate) fn put_in_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    source: &str,
    ast: &papyrus_parser::ast::Script,
    linter_version: &str,
) {
    put_field_for_game(
        dir,
        game,
        source_path,
        source,
        FieldUpdate::Ast(ast),
        linter_version,
    );
}

pub(crate) fn put_tokens_in_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    source: &str,
    tokens: &[papyrus_parser::token::Token],
    linter_version: &str,
) {
    put_field_for_game(
        dir,
        game,
        source_path,
        source,
        FieldUpdate::Tokens(tokens),
        linter_version,
    );
}

fn write_stamped_entry_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    source: &str,
    linter_version: &str,
    ast: Option<papyrus_parser::ast::Script>,
    tokens: Option<Vec<papyrus_parser::token::Token>>,
) {
    let Some(modified_unix_secs) = file_modified_unix_secs(source_path) else {
        return;
    };
    let entry = CacheEntry {
        modified_unix_secs,
        content_md5: format!("{:x}", md5::compute(source.as_bytes())),
        linter_version: linter_version.to_string(),
        ast,
        tokens,
    };
    write_entry_in_for_game(dir, game, source_path, &entry);
}

/// Persists `content_md5` against `source_path`'s current mtime. A still
/// mtime-fresh entry keeps its AST/tokens and stamped version; otherwise a
/// hash-only entry is written so later lookups can skip opening the `.psc`.
pub(crate) fn put_content_md5_in_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    content_md5: &str,
    linter_version: &str,
) {
    if let Some(existing) = mtime_valid_entry_in_for_game(dir, game, source_path) {
        if existing.content_md5 == content_md5 {
            return;
        }
        write_entry_in_for_game(
            dir,
            game,
            source_path,
            &CacheEntry {
                content_md5: content_md5.to_string(),
                ..existing
            },
        );
        return;
    }
    let Some(modified_unix_secs) = file_modified_unix_secs(source_path) else {
        return;
    };
    write_entry_in_for_game(
        dir,
        game,
        source_path,
        &CacheEntry {
            modified_unix_secs,
            content_md5: content_md5.to_string(),
            linter_version: linter_version.to_string(),
            ast: None,
            tokens: None,
        },
    );
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
