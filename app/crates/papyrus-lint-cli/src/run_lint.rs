//! Lints one already-resolved script source against the rest of the
//! project — the step every run does for each script, whether or not `fix`
//! ran first (see [`crate::run_fix`]).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use papyrus_lint_core::content_hash;
use papyrus_lint_core::function_table::{FunctionTable, SharedFunctionTable};
use papyrus_lint_core::ignore_file::IgnoreFile;
use papyrus_lint_core::project_lint::{lint_script, ConflictScope, ProjectLint};

use crate::output::*;

/// Everything [`lint_file`] needs about the project besides the one script
/// it's linting, shared read-only across every script in a run (and, with
/// `--threads`, across every worker thread linting one).
pub(crate) struct LintContext<'a> {
    pub(crate) lint_config: &'a papyrus_lints::Config,
    pub(crate) function_table: &'a RwLock<FunctionTable>,
    pub(crate) function_table_additional_roots: &'a [String],
    /// Achlist entries grouped by (lowercased) file name, for
    /// `conflicting_script_versions` in `strict_achlist_scope` mode (see
    /// [`crate::run_scan::ScanOutcome`]).
    pub(crate) scripts_by_name: &'a HashMap<String, Vec<PathBuf>>,
    pub(crate) script_index: &'a HashMap<String, Vec<PathBuf>>,
    /// The project root `conflicting_script_versions_among`/`_in_index`
    /// shorten a conflict's reported path against when `short_paths` is set.
    pub(crate) project_root: &'a Path,
    pub(crate) short_paths: bool,
    pub(crate) strict_achlist_scope: bool,
    pub(crate) compile_check: bool,
    pub(crate) compiler_path: &'a str,
    pub(crate) tag_filter: Option<&'a str>,
    pub(crate) quiet_warnings: bool,
    pub(crate) quiet_info: bool,
    pub(crate) json: bool,
    pub(crate) output_format: OutputFormat,
    pub(crate) hash_source: bool,
    pub(crate) use_color: bool,
    pub(crate) ignores: Option<&'a IgnoreFile>,
}

/// One script's lint result, ready to be folded into its [`FileOutcome`]
/// alongside whatever [`crate::run_fix::fix_file`] already contributed
/// (its own diff text, and whether it changed the file).
pub(crate) struct LintFileOutcome {
    /// This file's diagnostic lines, empty in JSON/AI mode.
    pub(crate) plain_text: Vec<u8>,
    pub(crate) json_file: Option<JsonFileReport>,
    pub(crate) ai_file: Option<AiFileReport>,
    pub(crate) parse_failed: bool,
    pub(crate) should_fail: bool,
    pub(crate) has_diagnostics: bool,
    pub(crate) diagnostic_count: usize,
}

/// Lints `source` (already read, and already fixed if `fix` was given) for
/// `script_path`, reported under `reported_path`. `file_diff` is
/// [`crate::run_fix::fix_file`]'s unified diff, if any, carried through into
/// this file's `--json` report as-is.
///
/// `already_primed` is true when the caller has already primed
/// `papyrus_parser`'s in-memory memoization for this exact `source` --
/// typically from a "parse every file first" pass's own in-memory AST/token
/// store (see `crate::run_lint_command`), rather than this call touching
/// `ast_cache`'s disk cache (and its process-wide lock) again for a source
/// string it already knows is unchanged.
pub(crate) fn lint_file(
    ctx: &LintContext,
    script_path: &Path,
    reported_path: String,
    source: &str,
    file_diff: Option<String>,
    already_primed: bool,
) -> LintFileOutcome {
    let mut shared = SharedFunctionTable(ctx.function_table);
    let mut diagnostics = lint_script(
        script_path,
        source,
        &mut shared,
        &ProjectLint {
            config: ctx.lint_config,
            project_root: ctx.project_root,
            additional_roots: ctx.function_table_additional_roots,
            conflicts: conflict_scope(ctx, script_path),
            compile_check: ctx.compile_check,
            compiler_path: ctx.compiler_path,
            ignores: ctx.ignores,
            already_primed,
            flush_collision_cache: false,
        },
    );
    let parser_errors = collect_parser_errors(source, ctx.lint_config.game);
    let parse_failed = !parser_errors.is_empty();
    let should_fail = finalize_diagnostics(
        &mut diagnostics,
        ctx.lint_config,
        ctx.tag_filter,
        ctx.quiet_warnings,
        ctx.quiet_info,
    );

    let (plain_text, json_file, ai_file) = build_file_reports(
        ctx,
        &reported_path,
        source,
        file_diff,
        &diagnostics,
        parser_errors,
    );

    let has_diagnostics = !diagnostics.is_empty();
    let diagnostic_count = diagnostics.len();

    LintFileOutcome {
        plain_text,
        json_file,
        ai_file,
        parse_failed,
        should_fail,
        has_diagnostics,
        diagnostic_count,
    }
}

fn conflict_scope<'a>(ctx: &'a LintContext<'_>, script_path: &Path) -> ConflictScope<'a> {
    if ctx.strict_achlist_scope {
        let candidates = script_path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| ctx.scripts_by_name.get(&name.to_ascii_lowercase()))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        ConflictScope::Known {
            candidates,
            short_paths: ctx.short_paths,
        }
    } else {
        ConflictScope::Index {
            index: ctx.script_index,
            short_paths: ctx.short_paths,
        }
    }
}

fn build_file_reports(
    ctx: &LintContext,
    reported_path: &str,
    source: &str,
    file_diff: Option<String>,
    diagnostics: &[papyrus_lints::Diagnostic],
    parser_errors: Vec<JsonParserError>,
) -> (Vec<u8>, Option<JsonFileReport>, Option<AiFileReport>) {
    let mut plain_text: Vec<u8> = Vec::new();
    if !ctx.json {
        for error in &parser_errors {
            let _ = writeln!(
                plain_text,
                "{}",
                format_parser_error_line(reported_path, error, ctx.use_color)
            );
        }
        for diagnostic in diagnostics {
            let _ = writeln!(
                plain_text,
                "{}",
                format_diagnostic_line(reported_path, diagnostic, ctx.use_color)
            );
        }
    }

    let mut json_file = None;
    let mut ai_file = None;
    if ctx.json {
        let json_diagnostics = to_json_diagnostics(diagnostics, false);
        if ctx.output_format == OutputFormat::Ai
            && (!json_diagnostics.is_empty() || !parser_errors.is_empty())
        {
            let rule_counts = rule_counts(&json_diagnostics);
            let severity_counts = severity_counts(&json_diagnostics);
            let ai_source = if ctx.hash_source {
                AiSource::Hash {
                    algorithm: "md5".to_string(),
                    hash: content_hash::md5_hex(source),
                }
            } else {
                AiSource::Content {
                    content: source.to_string(),
                }
            };
            ai_file = Some(AiFileReport {
                path: reported_path.to_string(),
                severity_counts,
                rule_counts,
                diagnostics: json_diagnostics,
                parser_errors,
                source: Some(ai_source),
            });
        } else if ctx.output_format == OutputFormat::Json {
            json_file = Some(JsonFileReport {
                path: reported_path.to_string(),
                diagnostics: json_diagnostics,
                parser_errors,
                diff: file_diff,
            });
        }
    }
    (plain_text, json_file, ai_file)
}
