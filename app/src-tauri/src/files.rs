//! File listing, reading, writing, hashing, and in-memory parse/lint commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{achlist, ast_cache, content_hash, ppj, script_locator};

/// Parses the `.achlist` file at `path` and returns the resolved paths it lists.
#[tauri::command(async)]
pub(crate) fn parse_achlist_file(path: String) -> Result<Vec<String>, String> {
    let entries = achlist::parse_achlist(&PathBuf::from(path)).map_err(|err| err.to_string())?;

    Ok(entries
        .into_iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect())
}

/// A parsed `.ppj`'s own `.psc` entries and `<Import>` search paths, for the
/// frontend's ppj-drop mode (mirroring the CLI's `scan_project`, see
/// `papyrus_lint_cli::run_scan::collect_script_paths`). `imports` are the
/// project's `additional_script_roots` equivalent — passed back so the
/// frontend can add them to its own script roots the same way it does for an
/// achlist's inferred directories (see `setAchlistScriptRoots`).
#[derive(Debug, PartialEq, serde::Serialize)]
pub(crate) struct PpjParseResult {
    scripts: Vec<String>,
    imports: Vec<String>,
}

/// Parses the `.ppj` file at `path` and returns the `.psc` files it compiles
/// alongside its own `<Import>` search paths.
#[tauri::command(async)]
pub(crate) fn parse_ppj_file(path: String) -> Result<PpjParseResult, String> {
    let project = ppj::parse_ppj(&PathBuf::from(path)).map_err(|err| err.to_string())?;

    let scripts = project
        .scripts
        .into_iter()
        .filter(|script| {
            script
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
        })
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect();
    let imports = project
        .imports
        .into_iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect();

    Ok(PpjParseResult { scripts, imports })
}

/// Recursively scans `dir` (and every subdirectory beneath it, at any
/// depth) for `.psc` files and returns their paths, for the frontend's
/// directory-drop mode — dropping a folder instead of an `.achlist`, useful
/// for a project (e.g. Requiem's own layout) whose scripts are spread
/// across arbitrarily nested subfolders rather than a flat
/// `scripts/source`. Returns an error if `path` isn't an existing
/// directory, so the frontend can fall back to its usual
/// "drop a single .achlist or .psc file" error.
#[tauri::command(async)]
pub(crate) fn list_psc_files_recursively(path: String) -> Result<Vec<String>, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("{path} is not a directory"));
    }

    Ok(script_locator::find_psc_files_recursively(&dir)
        .into_iter()
        .map(|entry| entry.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command(async)]
pub(crate) fn parse_papyrus_script(source: &str) -> Result<papyrus_parser::ast::Script, String> {
    papyrus_parser::parse(source).map_err(|e| e.to_string())
}

/// Lints an in-memory buffer the same way CLI `--blob` and the LSP snapshot
/// do: [`papyrus_lint_live::lint_source`], no project-level machinery.
/// `config` is the desktop app's current settings (including unsaved
/// Settings-tab toggles), not a file walk.
#[tauri::command(async)]
pub(crate) fn lint_papyrus_script(
    source: &str,
    config: papyrus_lints::Config,
) -> Vec<papyrus_lints::Diagnostic> {
    papyrus_lint_live::lint_source(source, &config).diagnostics
}

/// Reads the `.psc` file at `path` and parses it into a `Script` AST,
/// reusing a disk-backed cache (see [`ast_cache`]) keyed by `path`'s
/// content and modification time when the file hasn't changed since it was
/// last parsed.
#[tauri::command(async)]
pub(crate) fn parse_psc_file(
    path: String,
    game: papyrus_lints::Game,
) -> Result<papyrus_parser::ast::Script, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;

    if let Some(cached) = ast_cache::get_for_game(game, path, &source) {
        return Ok(cached);
    }

    let script = papyrus_parser::parse(&source).map_err(|err| err.to_string())?;
    ast_cache::put_for_game(game, path, &source, &script);
    if let Ok(tokens) = papyrus_parser::tokenize(&source) {
        ast_cache::put_tokens_for_game(game, path, &source, &tokens);
    }
    Ok(script)
}

/// Reads the `.psc` file at `path` and returns its raw source text, for the
/// frontend's syntax-highlighted code viewer.
#[tauri::command(async)]
pub(crate) fn read_psc_file(path: String) -> Result<String, String> {
    read_psc_source(Path::new(&path)).map_err(|err| err.to_string())
}

/// Reads the `.psc` file at `path` and returns the lowercase hex MD5 digest
/// of its contents, for the "Export for AI" feature's "Redact source"
/// option (see `formatIssuesForAi`/`readIssueFileSources` in
/// `app/src/main.ts`): an assistant can still tell files apart, or notice a
/// file changed between exports, from this hash without seeing its actual
/// source text.
#[tauri::command(async)]
pub(crate) fn hash_psc_file_md5(path: String) -> Result<String, String> {
    let source = read_psc_source(Path::new(&path)).map_err(|err| err.to_string())?;
    Ok(content_hash::md5_hex(&source))
}

/// Returns each existing path in `paths`' own last-modified time, as Unix
/// milliseconds, keyed by that same path. Used by the frontend's watch mode
/// (see `watch.ts`) to notice a currently loaded `.psc` file changing on
/// disk without an OS-level filesystem-watcher dependency: it polls this
/// command for the files it's watching and compares the returned timestamps
/// against what it saw last time. A path whose metadata can't be read (e.g.
/// deleted, or momentarily locked by whatever wrote it) is simply omitted
/// from the result instead of failing the whole call, so one such file
/// doesn't stop watch mode from noticing changes to the rest — the frontend
/// treats that omission as a change in its own right, since a watched file
/// disappearing is itself worth re-linting (and reporting as an error) for.
#[tauri::command(async)]
pub(crate) fn get_psc_file_mtimes(paths: Vec<String>) -> HashMap<String, u64> {
    paths
        .into_iter()
        .filter_map(|path| {
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            let millis = modified.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
            Some((path, millis))
        })
        .collect()
}

/// Writes `contents` to the `.psc` file at `path`, replacing it on disk.
/// Used by the frontend's code viewer to persist edits made in its edit
/// mode.
#[tauri::command(async)]
pub(crate) fn write_psc_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, contents).map_err(|err| err.to_string())
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
