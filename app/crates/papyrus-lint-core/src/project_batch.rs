//! Batch parse-closure, function-table preload, and parallel lint walk
//! shared by the CLI and the desktop app.
//!
//! Stdout progress, fix-before-lint, and report folding stay in the CLI.
//! Tauri channels, and the desktop rule that an unparseable script is not
//! linted, stay in the app. The per-seed callback is where that caller
//! invokes [`crate::project_lint::lint_script`].

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use papyrus_parser::ast::Script;
use papyrus_parser::token::Token;

use crate::function_table::{ClosedScripts, FunctionTable, PreloadedScript, TypeClosureOptions};

/// How [`parse_closure`] reports each on-disk file and how many workers
/// parse them.
pub struct ClosureOptions {
    /// Worker count. `0` or `1` parses on the calling thread, matching
    /// [`FunctionTable::parse_type_closure`].
    pub threads: usize,
    /// When set, [`ClosureProgress::total`] grows as referenced `.psc`
    /// files are enqueued. Off, the total stays at the seed count.
    /// Bundled names are not counted: they are not disk parses.
    pub track_total: bool,
}

/// One on-disk parse finished inside [`parse_closure`]. `completed` counts
/// seeds and dependencies. `total` starts at the seed count and, when
/// [`ClosureOptions::track_total`] is set, grows before this callback runs.
pub struct ClosureProgress {
    pub completed: usize,
    pub total: usize,
}

/// Source and AST a seed contributes to [`FunctionTable::preload`].
/// Returning `None` from the seed callback skips that seed (it could not
/// be read). An unparseable script still preloads when `source` is present
/// and `ast` is `None`, matching the cached-unresolved slot a later
/// on-demand load would record.
pub struct SeedScript<'a> {
    pub source: &'a str,
    pub ast: Option<&'a Script>,
}

/// AST and tokens already parsed for `source`. [`lint_in_parallel`] copies
/// them into `papyrus_parser`'s thread-local memo before the lint callback,
/// so that pass does not take [`crate::ast_cache`]'s lock again. Empty
/// tokens are not primed: [`papyrus_parser::prime_tokenize_cache`] rejects
/// them.
pub struct ParsedMemo<'a> {
    pub source: &'a str,
    pub ast: Option<&'a Script>,
    pub tokens: Option<&'a [Token]>,
}

/// Parses every seed, then every script a type name in those ASTs resolves
/// to. `on_file` runs after each on-disk parse (seeds and dependencies, not
/// bundled names), on the worker that parsed it.
pub fn parse_closure<T: Send>(
    table: &FunctionTable,
    script_paths: &[PathBuf],
    options: ClosureOptions,
    parse_seed: impl Fn(&Path) -> T + Sync,
    seed_ast: impl Fn(&T) -> Option<&Script> + Sync,
    on_file: impl Fn(ClosureProgress) + Sync,
) -> ClosedScripts<T> {
    let total_files = AtomicUsize::new(script_paths.len());
    let completed = AtomicUsize::new(0);
    table.parse_type_closure(
        script_paths,
        TypeClosureOptions {
            threads: options.threads,
            total_files: options.track_total.then_some(&total_files),
        },
        parse_seed,
        seed_ast,
        || {
            let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
            on_file(ClosureProgress {
                completed: done,
                total: total_files.load(Ordering::SeqCst),
            });
        },
    )
}

/// Merges seeds, dependency parses, and bundled / unresolved name slots
/// into `table`. Seed order follows `script_paths`. A seed the callback
/// skips is left for a later [`FunctionTable::ensure_loaded`].
pub fn preload_closure<T>(
    table: &mut FunctionTable,
    script_paths: &[PathBuf],
    closed: &ClosedScripts<T>,
    seed_script: impl for<'a> Fn(&'a T) -> Option<SeedScript<'a>>,
) {
    let entries: Vec<PreloadedScript<'_>> = script_paths
        .iter()
        .zip(closed.seeds.iter())
        .filter_map(|(path, seed)| {
            let script = seed_script(seed)?;
            let name_lower = path.file_stem()?.to_str()?.to_ascii_lowercase();
            Some(PreloadedScript {
                path,
                name_lower,
                ast: script.ast,
                source: script.source,
            })
        })
        .collect();
    table.preload(entries);
    table.preload_dependencies(&closed.dependencies);
    table.preload_name_slots(&closed.bundled, &closed.unresolved);
}

/// Runs `lint_seed` once per seed, in seed order, after priming that
/// worker's parser memo from `memo`. `memo` returning `None` skips the
/// prime (the seed was not read) and still calls `lint_seed`.
///
/// `lint_seed` is the caller's per-file edge. It should call
/// [`crate::project_lint::lint_script`] for the source it actually lints
/// (after fix-before-lint, when the caller does that).
pub fn lint_in_parallel<T, R>(
    seeds: &[T],
    threads: usize,
    memo: impl for<'a> Fn(&'a T) -> Option<ParsedMemo<'a>> + Sync,
    lint_seed: impl Fn(usize, &T) -> R + Sync + Send,
) -> Vec<R>
where
    T: Sync,
    R: Send,
{
    crate::parallel::map_in_parallel((0..seeds.len()).collect(), threads, |index| {
        if let Some(parsed) = memo(&seeds[index]) {
            prime_parsed_memo(parsed);
        }
        lint_seed(index, &seeds[index])
    })
}

fn prime_parsed_memo(parsed: ParsedMemo<'_>) {
    if let Some(ast) = parsed.ast.cloned() {
        papyrus_parser::prime_cache(parsed.source, ast);
    }
    if let Some(tokens) = parsed.tokens {
        if !tokens.is_empty() {
            papyrus_parser::prime_tokenize_cache(parsed.source, tokens.to_vec());
        }
    }
}

#[cfg(test)]
#[path = "project_batch_tests.rs"]
mod tests;
