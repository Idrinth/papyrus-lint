//! The write/persistence half of [`super`]: stamping and writing a cache
//! entry, preserving whichever of the AST/tokens fields isn't being written
//! this call.

use std::path::Path;

use crate::entry::{
    file_modified_unix_secs, valid_entry_in, valid_entry_in_for_game, write_entry_in,
    write_entry_in_for_game, CacheEntry,
};

pub(crate) fn put_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
    ast: &papyrus_parser::ast::Script,
    linter_version: &str,
) {
    let tokens = valid_entry_in(dir, source_path, source).and_then(|entry| entry.tokens);
    write_stamped_entry(
        dir,
        source_path,
        source,
        linter_version,
        Some(ast.clone()),
        tokens,
    );
}

pub(crate) fn put_in_for_game(
    dir: &Path,
    game: &str,
    source_path: &Path,
    source: &str,
    ast: &papyrus_parser::ast::Script,
    linter_version: &str,
) {
    let tokens =
        valid_entry_in_for_game(dir, game, source_path, source).and_then(|entry| entry.tokens);
    write_stamped_entry_for_game(
        dir,
        game,
        source_path,
        source,
        linter_version,
        Some(ast.clone()),
        tokens,
    );
}

pub(crate) fn put_tokens_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
    tokens: &[papyrus_parser::token::Token],
    linter_version: &str,
) {
    let ast = valid_entry_in(dir, source_path, source).and_then(|entry| entry.ast);
    write_stamped_entry(
        dir,
        source_path,
        source,
        linter_version,
        ast,
        Some(tokens.to_vec()),
    );
}

pub(crate) fn put_tokens_in_for_game(
    dir: &Path,
    game: &str,
    source_path: &Path,
    source: &str,
    tokens: &[papyrus_parser::token::Token],
    linter_version: &str,
) {
    let ast = valid_entry_in_for_game(dir, game, source_path, source).and_then(|entry| entry.ast);
    write_stamped_entry_for_game(
        dir,
        game,
        source_path,
        source,
        linter_version,
        ast,
        Some(tokens.to_vec()),
    );
}

fn write_stamped_entry_for_game(
    dir: &Path,
    game: &str,
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

fn write_stamped_entry(
    dir: &Path,
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
    write_entry_in(dir, source_path, &entry);
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
