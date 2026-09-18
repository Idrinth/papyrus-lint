//! Applies automatic fixes to one script's source — the mutating half of
//! `fix` mode, run before [`crate::run_lint::lint_file`] lints whatever
//! source comes out of it.

use std::io::Write;
use std::path::Path;
use std::sync::RwLock;

use papyrus_lint_core::diff::unified_diff;
use papyrus_lint_core::function_table::{FunctionTable, SharedFunctionTable};
use papyrus_lint_core::source_encoding::{write_psc_source, PscEncoding};

/// The result of attempting to fix one script's source.
pub(crate) struct FixOutcome {
    /// The source to lint afterward: the original source if nothing
    /// changed, or the repaired text otherwise — even under `--dry-run`,
    /// so the diagnostics reported afterward reflect what the fix would
    /// leave behind.
    pub(crate) source: String,
    /// Whether the fix actually changed the file (or, under `--dry-run`,
    /// would have).
    pub(crate) fixed: bool,
    /// The unified diff between the original and repaired source, set only
    /// when it changed. Carried into this file's `--json` report as-is.
    pub(crate) diff: Option<String>,
    /// In plain-text mode, the same diff formatted for the report (empty
    /// otherwise, and whenever nothing changed).
    pub(crate) plain_text: Vec<u8>,
}

/// Applies every enabled automatic fix (or, with `rule_filter`/`tag_filter`,
/// just the matching one(s)) to `source`, restricting to `target_line` if
/// given. With `dry_run`, nothing is written to `script_path`; otherwise the
/// repaired source is written back (preserving `encoding`) when it changed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fix_file(
    script_path: &Path,
    reported_path: &str,
    source: String,
    encoding: PscEncoding,
    lint_config: &papyrus_lints::Config,
    function_table: &RwLock<FunctionTable>,
    tag_filter: Option<&str>,
    rule_filter: Option<&'static str>,
    target_line: Option<usize>,
    dry_run: bool,
    json: bool,
) -> Result<FixOutcome, String> {
    // The "unused-import" fix, unlike every other fixable rule, can only
    // resolve which imports are actually unused through this project's own
    // cross-script resolver -- the same `SharedFunctionTable` the lint pass
    // uses -- so the fix step needs one too, via `papyrus_lints`'
    // `_with_external_arguments` repair family (see that module's own
    // docs).
    let mut shared = SharedFunctionTable(function_table);
    let repaired = papyrus_lints::repair_selected_with_external_arguments(
        &source,
        lint_config,
        &mut shared,
        rule_filter,
        tag_filter,
        target_line,
    )
    .ok_or_else(|| {
        format!(
            "error: --line can't be applied to {} because a fix changes the file's line count (e.g. property-sorting); use --type to restrict to a line-preserving fix, or omit --line",
            script_path.display()
        )
    })?;

    let mut plain_text: Vec<u8> = Vec::new();
    let mut diff = None;
    let mut fixed = false;
    if repaired != source {
        if dry_run {
            let diff_text = unified_diff(reported_path, &source, &repaired);
            if !json {
                let _ = write!(plain_text, "{diff_text}");
            }
            diff = Some(diff_text);
        } else {
            write_psc_source(script_path, &repaired, encoding).map_err(|err| {
                format!("error: failed to write {}: {err}", script_path.display())
            })?;
        }
        fixed = true;
    }

    Ok(FixOutcome {
        source: repaired,
        fixed,
        diff,
        plain_text,
    })
}
