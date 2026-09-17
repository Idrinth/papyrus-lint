//! Runs the plain lint/fix pipeline for one invocation, end to end: scans
//! the project (see [`crate::run_scan::scan_project`]), processes every
//! resolved script — fixing it first when `fix` was given (see
//! [`crate::run_fix::fix_file`]) and then linting whatever source comes out
//! of that (see [`crate::run_lint::lint_file`]) — optionally in parallel via
//! `--threads`, and folds the results into the final report (see
//! [`crate::report::fold_and_flush_report`]).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use papyrus_lint_core::source_encoding::read_psc_source_with_encoding;

use crate::args::LintArgs;
use crate::output::*;
use crate::project::display_path;
use crate::report::fold_and_flush_report;
use crate::run_fix::fix_file;
use crate::run_lint::{lint_file, LintContext};
use crate::run_scan::scan_project;

/// Runs a plain lint/fix invocation end to end: scans `lint`'s target
/// project (see [`scan_project`]), fixes and lints every resolved script --
/// optionally in parallel via `--threads` -- then folds the results into
/// the final report (see [`fold_and_flush_report`]). Returns the process
/// exit code (see [`crate::run`]'s own docs for what each code means).
pub(crate) fn run_lint_command(
    lint: LintArgs,
    stdout: &mut (impl Write + Send),
    stderr: &mut impl Write,
    stdout_is_terminal: bool,
) -> u8 {
    let scan = match scan_project(
        &lint.input_path,
        lint.config_path.as_deref(),
        lint.cli_script_roots,
    ) {
        Ok(scan) => scan,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };

    // `--output <path>` redirects the report to a file, which is never a
    // terminal, so `auto` never colorizes in that case regardless of
    // whether `stdout` itself is one. `NO_COLOR` (https://no-color.org/)
    // is honored the same way most CLIs do: any non-empty or empty value
    // disables auto-coloring.
    let use_color = resolve_color(
        lint.color_choice,
        lint.output_path.as_deref(),
        stdout_is_terminal,
    );

    let json = lint.output_format != OutputFormat::Plain;
    let total_scripts = scan.script_paths.len();
    let function_table_root = scan.function_table.root().to_path_buf();
    let function_table_additional_roots = scan.function_table.additional_roots().to_vec();
    // Every script is otherwise independent, so this table's own cache
    // (of other scripts' cross-referenced signatures) is the only thing
    // `--threads` workers below actually share -- through
    // `SharedFunctionTable`, which locks it only for the duration of a
    // single lookup rather than a whole script's lint pass. Progress
    // ("--progress") is likewise reported through a shared counter rather
    // than each worker's own position in `script_paths`, since completion
    // order no longer matches input order once more than one thread is
    // involved -- the final report still is, via `map_in_parallel`'s
    // ordering guarantee.
    let function_table = Mutex::new(scan.function_table);
    let progress_completed = AtomicUsize::new(0);
    let stdout_dyn: &mut (dyn Write + Send) = stdout;
    let progress_stdout: Mutex<&mut (dyn Write + Send)> = Mutex::new(stdout_dyn);

    // Shared read-only across every script (and, with `--threads`, across
    // every worker thread linting one) -- see `run_lint::LintContext`.
    let lint_context = LintContext {
        lint_config: &scan.lint_config,
        function_table: &function_table,
        function_table_additional_roots: &function_table_additional_roots,
        scripts_by_name: &scan.scripts_by_name,
        script_index: &scan.script_index,
        strict_achlist_scope: scan.strict_achlist_scope,
        compile_check: scan.compile_check,
        compiler_path: &scan.compiler_path,
        tag_filter: lint.tag_filter.as_deref(),
        quiet_warnings: lint.quiet_warnings,
        quiet_info: lint.quiet_info,
        json,
        output_format: lint.output_format,
        hash_source: lint.hash_source,
        use_color,
    };

    let file_results: Vec<Result<FileOutcome, String>> =
        papyrus_lint_core::parallel::map_in_parallel(
            (0..total_scripts).collect(),
            lint.thread_count,
            |file_index| {
                process_script(
                    &lint_context,
                    &scan.script_paths,
                    file_index,
                    &function_table_root,
                    lint.short_paths,
                    lint.fix,
                    lint.rule_filter,
                    lint.target_line,
                    lint.dry_run,
                    lint.progress,
                    &progress_completed,
                    total_scripts,
                    &progress_stdout,
                )
            },
        );

    if let Some(message) = file_results.iter().find_map(|result| result.as_ref().err()) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }

    let stdout = progress_stdout
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    fold_and_flush_report(
        file_results,
        lint.output_format,
        &scan.lint_config,
        scan.script_paths.len(),
        lint.fix,
        lint.dry_run,
        use_color,
        lint.progress,
        lint.output_path.as_deref(),
        stdout,
        stderr,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_script(
    lint_context: &LintContext,
    script_paths: &[PathBuf],
    file_index: usize,
    function_table_root: &Path,
    short_paths: bool,
    fix: bool,
    rule_filter: Option<&'static str>,
    target_line: Option<usize>,
    dry_run: bool,
    progress: bool,
    progress_completed: &AtomicUsize,
    total_scripts: usize,
    progress_stdout: &Mutex<&mut (dyn Write + Send)>,
) -> Result<FileOutcome, String> {
    let script_path = &script_paths[file_index];
    let (source, encoding) = read_psc_source_with_encoding(script_path)
        .map_err(|err| format!("error: failed to read {}: {err}", script_path.display()))?;

    let reported_path = display_path(script_path, function_table_root, short_paths);

    // `fix` mutates (or, under `--dry-run`, previews) the source first;
    // `run_lint::lint_file` then lints whatever source comes out of that
    // (the original source, if `fix` didn't run or changed nothing).
    let (source, fixed_this_file, file_diff, mut plain_text) = if fix {
        let outcome = fix_file(
            script_path,
            &reported_path,
            source,
            encoding,
            lint_context.lint_config,
            lint_context.function_table,
            lint_context.tag_filter,
            rule_filter,
            target_line,
            dry_run,
            lint_context.json,
        )?;
        (
            outcome.source,
            outcome.fixed,
            outcome.diff,
            outcome.plain_text,
        )
    } else {
        (source, false, None, Vec::new())
    };

    let lint_outcome = lint_file(lint_context, script_path, reported_path, &source, file_diff);
    plain_text.extend_from_slice(&lint_outcome.plain_text);

    if progress {
        report_file_progress(progress_completed, total_scripts, progress_stdout);
    }

    Ok(FileOutcome {
        plain_text,
        json_file: lint_outcome.json_file,
        ai_file: lint_outcome.ai_file,
        should_fail: lint_outcome.should_fail,
        has_diagnostics: lint_outcome.has_diagnostics,
        diagnostic_count: lint_outcome.diagnostic_count,
        fixed: fixed_this_file,
    })
}

fn report_file_progress(
    progress_completed: &AtomicUsize,
    total_scripts: usize,
    progress_stdout: &Mutex<&mut (dyn Write + Send)>,
) {
    let completed = progress_completed.fetch_add(1, Ordering::SeqCst) + 1;
    let mut stdout = progress_stdout
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = write!(stdout, "\rLinting: {completed}/{total_scripts} files");
    let _ = stdout.flush();
}
