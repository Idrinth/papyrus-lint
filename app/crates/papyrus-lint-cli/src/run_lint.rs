//! Lints one already-resolved script source against the rest of the
//! project — the step every run does for each script, whether or not `fix`
//! ran first (see [`crate::run_fix`]).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use papyrus_lint_core::function_table::{FunctionTable, SharedFunctionTable};
use papyrus_lint_core::{ast_cache, compile_diagnostics, compiler, content_hash};

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
    if !already_primed {
        ast_cache::ensure_primed_for_game(ctx.lint_config.game, script_path, source);
    }
    // Computed up front and merged in via
    // `lint_with_external_arguments_and_extra_diagnostics` below, rather
    // than appended to that call's own result afterward, so a
    // `@disable`/`@disable-file` directive naming one of these
    // path-dependent diagnostics is honored *and* counted as used by the
    // `unused-disable` lint instead of being incorrectly flagged as unused
    // (see `papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`'s
    // own docs).
    let project_diagnostics = collect_project_diagnostics(ctx, script_path, source);
    let mut diagnostics = {
        let mut shared = SharedFunctionTable(ctx.function_table);
        papyrus_lints::lint_with_external_arguments_and_extra_diagnostics(
            source,
            ctx.lint_config,
            &mut shared,
            project_diagnostics,
        )
    };
    // Mirrors the desktop app's `lint_with_compile_check`: a
    // `compiler_path` that can't be run at all (missing/misconfigured) is
    // silently left out rather than failing the whole lint run.
    if ctx.compile_check && !ctx.compiler_path.is_empty() {
        if let Ok(outcome) = compiler::check_psc_file(
            ctx.lint_config.game,
            Path::new(ctx.compiler_path),
            script_path,
            ctx.function_table_additional_roots,
        ) {
            if !outcome.success {
                diagnostics.extend(compile_diagnostics::parse_compile_errors(&outcome));
            }
        }
    }
    let parser_errors = collect_parser_errors(source);
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

fn collect_project_diagnostics(
    ctx: &LintContext,
    script_path: &Path,
    source: &str,
) -> Vec<papyrus_lints::Diagnostic> {
    let mut project_diagnostics = Vec::new();
    if ctx.lint_config.rules.conflicting_script_versions {
        if ctx.strict_achlist_scope {
            // No directories were added to `additional_script_roots` in
            // this mode (see `crate::run_scan`), so
            // `conflicting_script_versions`'s own directory scan would find
            // nothing among achlist entries anyway; comparing the
            // achlist's own listed entries directly is what actually
            // catches a same-named collision here, without re-reporting one
            // directory scanning might otherwise also find (e.g. two
            // entries whose directories both also happen to be configured
            // `additional_script_roots`).
            if let Some(name) = script_path.file_name().and_then(|name| name.to_str()) {
                if let Some(same_named) = ctx.scripts_by_name.get(&name.to_ascii_lowercase()) {
                    project_diagnostics.extend(
                        papyrus_lint_core::script_locator::conflicting_script_versions_among(
                            script_path,
                            same_named,
                            ctx.project_root,
                            ctx.short_paths,
                        ),
                    );
                }
            }
        } else {
            project_diagnostics.extend(
                papyrus_lint_core::script_locator::conflicting_script_versions_in_index(
                    script_path,
                    ctx.script_index,
                    ctx.project_root,
                    ctx.short_paths,
                ),
            );
        }
    }
    if ctx.lint_config.rules.stale_compiled_output {
        project_diagnostics.extend(papyrus_lint_core::stale_pex::check(script_path));
    }
    if ctx.lint_config.rules.script_filename_mismatch {
        project_diagnostics.extend(papyrus_lint_core::script_filename_mismatch::check(
            script_path,
            source,
        ));
    }
    project_diagnostics
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
    for diagnostic in diagnostics {
        if !ctx.json {
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
