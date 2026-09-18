//! Repair, preview-repair, and per-line disable-comment commands.

use std::path::Path;

use papyrus_lint_core::ast_cache;
use papyrus_lint_core::source_encoding::{
    read_psc_source, read_psc_source_with_encoding, write_psc_source,
};

use crate::lint::{lint_with_compile_check, project_function_table};

/// Reads the `.psc` file at `path`, applies every automatic fix (honoring
/// the semicolon and indentation style `config` selects), writes the
/// repaired source back to disk, and returns the diagnostics that remain.
/// See [`lint_psc_file`] for `root`/`additional_roots`/`compiler_path`/
/// `compile_check`.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    // Built before the fix, rather than after it like `lint_psc_file`'s own
    // call, since "unused-import" -- unlike every other fixable rule -- can
    // only resolve which imports are unused through this project's own
    // cross-script resolver (see `papyrus_lints::repair_with_external_arguments`).
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    let repaired =
        papyrus_lints::repair_with_external_arguments(&source, &config, &mut function_table);
    if repaired != source {
        write_psc_source(path, &repaired, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &repaired);
    Ok(lint_with_compile_check(
        path,
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Like [`repair_psc_file`], but never writes anything to disk: computes the
/// same whole-file automatic fix and returns a standard unified diff (the
/// same `diff -u`/`git diff` hunk format `PapyrusLinterCLI fix --dry-run`
/// prints, via [`papyrus_lint_core::diff::unified_diff`]) between the
/// original source and what applying the fix would produce, or an empty
/// string if nothing would change. Drives the code viewer's "Preview
/// fixes" button, so a user can see what "Apply fixes" would do before
/// committing to it.
#[tauri::command(async)]
pub(crate) fn preview_repair_psc_file(
    path: String,
    config: papyrus_lints::Config,
) -> Result<String, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    let repaired = papyrus_lints::repair(&source, &config);
    Ok(papyrus_lint_core::diff::unified_diff(
        &path.display().to_string(),
        &source,
        &repaired,
    ))
}

/// Applies only the automatic fix for `rule` (a
/// [`papyrus_lints::FIXABLE_RULE_IDS`] id) and returns what `line`
/// (1-indexed) would look like afterward, via
/// [`papyrus_lints::repaired_line`], without writing anything to disk.
/// Returns `None` when there's nothing meaningful to preview: the fix
/// doesn't change the file at all, it would shift the line count elsewhere
/// (e.g. `property-sorting` relocating a property's declaration), or it
/// simply doesn't touch `line`. Drives the "Export for AI" document's
/// per-finding `repair` preview (see `formatIssuesForAi` in
/// `app/src/main.ts`), so an AI reading the export can see each
/// auto-fixable finding's fix without applying it first.
#[tauri::command(async)]
pub(crate) fn preview_repair_psc_line(
    path: String,
    config: papyrus_lints::Config,
    rule: String,
    line: usize,
) -> Result<Option<String>, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    Ok(papyrus_lints::repaired_line(&source, &config, &rule, line))
}

/// Like [`repair_psc_file`], but applies only the automatic fix for `rule`
/// (a [`papyrus_lints::FIXABLE_RULE_IDS`] id), and restricts its effect to
/// `line` (1-indexed) — leaving every other line untouched — via
/// [`papyrus_lints::restrict_to_line`]. Drives the frontend's per-finding
/// "Fix this issue" button. Fails if the named rule's fix would change
/// `line`'s line count elsewhere in the file (e.g. `property-sorting`
/// relocating a property's declaration), since a single original line
/// number then no longer identifies the same line in the result; the
/// frontend surfaces that error and points the user at "Apply fixes"
/// instead.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_psc_finding(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rule: String,
    line: usize,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    // See `repair_psc_file`'s own comment: built before the fix so
    // "unused-import"'s fix (if `rule` names it) can resolve through it too.
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    let repaired = papyrus_lints::repair_selected_with_external_arguments(
        &source,
        &config,
        &mut function_table,
        Some(rule.as_str()),
        None,
        Some(line),
    )
    .ok_or_else(|| {
        "Fixing this issue would change other lines in the file; use \"Apply fixes\" instead."
            .to_string()
    })?;
    if repaired != source {
        write_psc_source(path, &repaired, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &repaired);
    Ok(lint_with_compile_check(
        path,
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Like [`repair_psc_file`], but applies only the automatic fix for `rule`
/// (a [`papyrus_lints::FIXABLE_RULE_IDS`] id) across the whole file, rather
/// than every fixable rule. Unlike [`repair_psc_finding`], it isn't
/// restricted to a single line and can't fail on a line-count mismatch, so
/// it drives the frontend's "mass fix" action, which repeats this call
/// across every file in the current results to clear one issue project-wide
/// (e.g. every trailing-whitespace finding) in one go.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_psc_file_rule(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rule: String,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    // See `repair_psc_file`'s own comment: built before the fix so
    // "unused-import"'s fix (if `rule` names it) can resolve through it too.
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    let repaired = papyrus_lints::repair_filtered_with_external_arguments(
        &source,
        &config,
        &mut function_table,
        Some(rule.as_str()),
    );
    if repaired != source {
        write_psc_source(path, &repaired, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &repaired);
    Ok(lint_with_compile_check(
        path,
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Adds (or extends) an `; @disable <rules>` comment on `line` (1-indexed)
/// of the named `.psc` file, covering every id in `rules` — the code
/// viewer's per-line "Ignore" button, the inverse of
/// [`repair_psc_finding`]'s per-line "Fix": rather than fixing the findings
/// on that line, it silences them via
/// [`papyrus_lints::add_disable_comment`] instead. Re-lints the file
/// afterward and returns its updated diagnostics, the same as every other
/// mutating command here.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_disable_comment_to_psc_line(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rules: Vec<String>,
    line: usize,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    let updated = papyrus_lints::add_disable_comment(&source, line, &rules);
    if updated != source {
        write_psc_source(path, &updated, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &updated);
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    Ok(lint_with_compile_check(
        path,
        &updated,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

#[cfg(test)]
#[path = "repair_tests.rs"]
mod tests;
