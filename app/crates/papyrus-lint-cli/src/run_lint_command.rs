//! Runs the plain lint/fix pipeline for one invocation, end to end: scans
//! the project (see [`crate::run_scan::scan_project`]), parses every
//! resolved script up front (see [`parse_scripts`]), then processes each one
//! — fixing it first when `fix` was given (see [`crate::run_fix::fix_file`])
//! and then linting whatever source comes out of that (see
//! [`crate::run_lint::lint_file`]) — optionally in parallel via `--threads`,
//! and folds the results into the final report (see
//! [`crate::report::fold_and_flush_report`]).
//!
//! Parsing does not stop at the lint targets. When a script's AST names
//! another type, that type is resolved the same way
//! [`papyrus_lint_core::function_table::FunctionTable::ensure_loaded`] would
//! and, if it is a `.psc`, parsed too ([`FunctionTable::parse_type_closure`]).
//! Those extra scripts are preloaded for analysis only — they are not
//! linted. Bundled vanilla/extender scripts are loaded from the blob, and
//! names that resolve nowhere are cached unresolved, so the lint phase's
//! [`SharedFunctionTable`] takes its write lock only for a name the closure
//! never saw.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::sync::RwLock;

use papyrus_lint_core::ast_cache;
use papyrus_lint_core::function_table::{
    ClosedScripts, FunctionTable, PreloadedScript, TypeClosureOptions,
};
use papyrus_lint_core::source_encoding::{read_psc_source_with_encoding, PscEncoding};

use crate::args::LintArgs;
use crate::output::*;
use crate::project::display_path;
use crate::report::fold_and_flush_report;
use crate::run_fix::fix_file;
use crate::run_lint::{lint_file, LintContext};
use crate::run_scan::{scan_project, ScanOutcome};

/// Per-script knobs from [`LintArgs`] that [`process_script`] needs besides
/// the shared [`LintContext`]. Bundled so the worker isn't a 13-argument
/// function, and so `--progress` reporting can stay out of this record
/// (it is shared mutable state, not a per-script flag's value).
struct ScriptJob {
    file_index: usize,
    short_paths: bool,
    fix: bool,
    rule_filter: Option<&'static str>,
    target_line: Option<usize>,
    dry_run: bool,
    progress: bool,
}

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
    let mut lint = lint;
    let scan = match scan_project(
        &lint.input_path,
        lint.config_path.as_deref(),
        std::mem::take(&mut lint.cli_script_roots),
    ) {
        Ok(scan) => scan,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };

    // Color detection lives in `papyrus-lint-output::resolve_color`
    // (NO_COLOR / CLICOLOR / CLICOLOR_FORCE via `anstyle-query`). `--output
    // <path>` is never a terminal, so `auto` never colorizes in that case
    // regardless of whether `stdout` itself is one.
    let use_color = resolve_color(
        lint.color_choice,
        lint.output_path.as_deref(),
        stdout_is_terminal,
    );

    let scripts_checked = scan.script_paths.len();
    let lint_config = scan.lint_config.clone();
    let (file_results, stdout) = process_scripts(scan, &lint, use_color, stdout);

    if let Some(message) = file_results.iter().find_map(|result| result.as_ref().err()) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }

    fold_and_flush_report(
        file_results,
        lint.output_format,
        &lint_config,
        scripts_checked,
        lint.fix,
        lint.dry_run,
        use_color,
        lint.progress,
        lint.output_path.as_deref(),
        stdout,
        stderr,
    )
}

/// One project script already read and parsed by [`parse_scripts`], carried
/// forward into the lint phase so it never has to be read from disk or
/// parsed again there -- see this module's own docs.
struct ParsedFile {
    source: String,
    encoding: PscEncoding,
    ast: Option<papyrus_parser::ast::Script>,
    tokens: Option<Vec<papyrus_parser::token::Token>>,
}

struct PreparedSource {
    source: String,
    fixed: bool,
    diff: Option<String>,
    plain_text: Vec<u8>,
}

/// Reads and parses every lint target, then every script a type name in
/// those ASTs resolves to, optionally in parallel via `--threads`.
/// Reports "Parsing: n/total" as each on-disk file finishes when `progress`
/// is set; `total` grows as referenced `.psc` files are enqueued. Lint
/// targets keep their input order in [`ClosedScripts::seeds`]. Referenced
/// scripts that are not lint targets are returned separately and must not
/// be appended to `script_paths`.
fn parse_scripts(
    function_table: &FunctionTable,
    script_paths: &[PathBuf],
    game: papyrus_lints::Game,
    thread_count: usize,
    progress: bool,
    progress_stdout: &Mutex<&mut (dyn Write + Send)>,
) -> ClosedScripts<Result<ParsedFile, String>> {
    let total_scripts = AtomicUsize::new(script_paths.len());
    let progress_completed = AtomicUsize::new(0);
    function_table.parse_type_closure(
        script_paths,
        TypeClosureOptions {
            threads: thread_count,
            total_files: progress.then_some(&total_scripts),
        },
        |script_path| parse_script(game, script_path),
        |parsed| parsed.as_ref().ok().and_then(|parsed| parsed.ast.as_ref()),
        || {
            if progress {
                report_file_progress(
                    &progress_completed,
                    &total_scripts,
                    progress_stdout,
                    "Parsing",
                );
            }
        },
    )
}

fn parse_script(game: papyrus_lints::Game, script_path: &Path) -> Result<ParsedFile, String> {
    let (source, encoding) = read_psc_source_with_encoding(script_path)
        .map_err(|err| format!("error: failed to read {}: {err}", script_path.display()))?;
    ast_cache::ensure_primed_for_game(game, script_path, &source);
    let ast = ast_cache::get_for_game(game, script_path, &source);
    let tokens = ast_cache::get_tokens_for_game(game, script_path, &source);
    Ok(ParsedFile {
        source,
        encoding,
        ast,
        tokens,
    })
}

/// Merges every successfully parsed script's AST into `function_table`
/// ([`FunctionTable::preload`]), including referenced scripts that are not
/// lint targets, before it is shared read-write across lint workers.
/// Bundled names and names that resolved nowhere are cached too
/// ([`FunctionTable::preload_name_slots`]), so resolving one of them during
/// lint is a cache hit rather than a write-locked on-demand parse. A lint
/// target that failed to read has nothing to preload (its `Err` is
/// surfaced again, unchanged, when the lint phase re-indexes it); one that
/// read but failed to parse still contributes a cached-unresolved entry,
/// same as [`FunctionTable::ensure_loaded`] would cache for it on demand.
fn preload_function_table(
    function_table: &mut FunctionTable,
    script_paths: &[PathBuf],
    parsed: &ClosedScripts<Result<ParsedFile, String>>,
) {
    let entries: Vec<PreloadedScript> = script_paths
        .iter()
        .zip(parsed.seeds.iter())
        .filter_map(|(path, parsed)| {
            let parsed = parsed.as_ref().ok()?;
            let name_lower = path.file_stem()?.to_str()?.to_ascii_lowercase();
            Some(PreloadedScript {
                path,
                name_lower,
                ast: parsed.ast.as_ref(),
                source: &parsed.source,
            })
        })
        .collect();
    function_table.preload(entries);
    function_table.preload_dependencies(&parsed.dependencies);
    function_table.preload_name_slots(&parsed.bundled, &parsed.unresolved);
}

/// Parses every resolved script up front (see [`parse_scripts`]), preloads
/// `scan`'s function table from the result ([`preload_function_table`]),
/// shares it across workers, builds the read-only [`LintContext`], and
/// lints every resolved script (optionally in parallel). Returns the
/// per-script outcomes in original order, plus `stdout` after the progress
/// bar's lock is released so the caller can fold the report onto the same
/// writer.
fn process_scripts<'a>(
    scan: ScanOutcome,
    lint: &'a LintArgs,
    use_color: bool,
    stdout: &'a mut (dyn Write + Send),
) -> (Vec<Result<FileOutcome, String>>, &'a mut dyn Write) {
    let total_scripts = scan.script_paths.len();
    let ScanOutcome {
        script_paths,
        lint_config,
        mut function_table,
        scripts_by_name,
        script_index,
        strict_achlist_scope,
        compile_check,
        compiler_path,
    } = scan;

    let progress_stdout: Mutex<&mut (dyn Write + Send)> = Mutex::new(stdout);

    let parsed_files = parse_scripts(
        &function_table,
        &script_paths,
        lint_config.game,
        lint.thread_count,
        lint.progress,
        &progress_stdout,
    );
    if lint.progress && total_scripts > 0 {
        // Ends the "Parsing" bar's line so "Linting"'s own `\r`-updated one
        // starts fresh below it instead of overwriting it mid-word.
        let mut stdout = progress_stdout
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _ = writeln!(stdout);
    }

    preload_function_table(&mut function_table, &script_paths, &parsed_files);
    let (function_table_root, function_table_additional_roots, function_table) =
        share_function_table(function_table);

    let progress_completed = AtomicUsize::new(0);
    let lint_total = AtomicUsize::new(total_scripts);

    let lint_context = LintContext {
        lint_config: &lint_config,
        function_table: &function_table,
        function_table_additional_roots: &function_table_additional_roots,
        scripts_by_name: &scripts_by_name,
        script_index: &script_index,
        project_root: &function_table_root,
        short_paths: lint.short_paths,
        strict_achlist_scope,
        compile_check,
        compiler_path: &compiler_path,
        tag_filter: lint.tag_filter.as_deref(),
        quiet_warnings: lint.quiet_warnings,
        quiet_info: lint.quiet_info,
        json: lint.output_format != OutputFormat::Plain,
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
                    &script_paths,
                    &parsed_files,
                    &function_table_root,
                    ScriptJob {
                        file_index,
                        short_paths: lint.short_paths,
                        fix: lint.fix,
                        rule_filter: lint.rule_filter,
                        target_line: lint.target_line,
                        dry_run: lint.dry_run,
                        progress: lint.progress,
                    },
                    &progress_completed,
                    &lint_total,
                    &progress_stdout,
                )
            },
        );

    let stdout = progress_stdout
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    (file_results, stdout)
}

/// Every script is otherwise independent, so this table's own cache (of
/// other scripts' cross-referenced signatures) is the only thing
/// `--threads` workers actually share -- through `SharedFunctionTable`,
/// which, now that [`preload_function_table`] has already closed over the
/// type names the parse phase saw, takes a write lock only for a name that
/// closure missed. Progress ("--progress") is reported through a shared
/// counter rather than each worker's own position in `script_paths`, since
/// completion order no longer matches input order once more than one thread
/// is involved -- the final report still is, via `map_in_parallel`'s
/// ordering guarantee.
fn share_function_table(
    function_table: FunctionTable,
) -> (PathBuf, Vec<String>, RwLock<FunctionTable>) {
    (
        function_table.root().to_path_buf(),
        function_table.additional_roots().to_vec(),
        RwLock::new(function_table),
    )
}

#[allow(clippy::too_many_arguments)]
fn process_script(
    lint_context: &LintContext,
    script_paths: &[PathBuf],
    parsed_files: &ClosedScripts<Result<ParsedFile, String>>,
    function_table_root: &Path,
    job: ScriptJob,
    progress_completed: &AtomicUsize,
    total_scripts: &AtomicUsize,
    progress_stdout: &Mutex<&mut (dyn Write + Send)>,
) -> Result<FileOutcome, String> {
    let script_path = &script_paths[job.file_index];
    let parsed = parsed_files.seeds[job.file_index]
        .as_ref()
        .map_err(Clone::clone)?;

    let reported_path = display_path(script_path, function_table_root, job.short_paths);

    prime_parser_cache(parsed);
    let mut prepared = prepare_source(lint_context, script_path, &reported_path, parsed, &job)?;

    // `already_primed` is only trustworthy when `fix` left the source
    // exactly as parsed above; a fix that changed it falls back to
    // `lint_file`'s own `ast_cache::ensure_primed` for the new text.
    let lint_outcome = lint_file(
        lint_context,
        script_path,
        reported_path,
        &prepared.source,
        prepared.diff,
        !prepared.fixed,
    );
    prepared
        .plain_text
        .extend_from_slice(&lint_outcome.plain_text);

    if job.progress {
        report_file_progress(
            progress_completed,
            total_scripts,
            progress_stdout,
            "Linting",
        );
    }

    Ok(FileOutcome {
        plain_text: prepared.plain_text,
        json_file: lint_outcome.json_file,
        ai_file: lint_outcome.ai_file,
        parse_failed: lint_outcome.parse_failed,
        should_fail: lint_outcome.should_fail,
        has_diagnostics: lint_outcome.has_diagnostics,
        diagnostic_count: lint_outcome.diagnostic_count,
        fixed: prepared.fixed,
    })
}

fn prime_parser_cache(parsed: &ParsedFile) {
    // Primes `papyrus_parser`'s in-memory memoization from the parse
    // phase's own already-owned AST/tokens, so neither `fix_file`'s own
    // internal parse (below) nor `run_lint::lint_file`'s touches
    // `ast_cache`'s disk cache (and its process-wide lock) again for a
    // source string already known to be current.
    if let Some(ast) = parsed.ast.clone() {
        papyrus_parser::prime_cache(&parsed.source, ast);
    }
    if let Some(tokens) = parsed.tokens.clone() {
        papyrus_parser::prime_tokenize_cache(&parsed.source, tokens);
    }
}

fn prepare_source(
    lint_context: &LintContext,
    script_path: &Path,
    reported_path: &str,
    parsed: &ParsedFile,
    job: &ScriptJob,
) -> Result<PreparedSource, String> {
    // `fix` mutates (or, under `--dry-run`, previews) the source first;
    // `run_lint::lint_file` then lints whatever source comes out of that
    // (the original source, if `fix` didn't run or changed nothing).
    if job.fix {
        let outcome = fix_file(
            script_path,
            reported_path,
            parsed.source.clone(),
            parsed.encoding,
            lint_context.lint_config,
            lint_context.function_table,
            lint_context.tag_filter,
            job.rule_filter,
            job.target_line,
            job.dry_run,
            lint_context.json,
        )?;
        Ok(PreparedSource {
            source: outcome.source,
            fixed: outcome.fixed,
            diff: outcome.diff,
            plain_text: outcome.plain_text,
        })
    } else {
        Ok(PreparedSource {
            source: parsed.source.clone(),
            fixed: false,
            diff: None,
            plain_text: Vec::new(),
        })
    }
}

fn report_file_progress(
    progress_completed: &AtomicUsize,
    total_scripts: &AtomicUsize,
    progress_stdout: &Mutex<&mut (dyn Write + Send)>,
    label: &str,
) {
    let completed = progress_completed.fetch_add(1, Ordering::SeqCst) + 1;
    let mut stdout = progress_stdout
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let total_scripts = total_scripts.load(Ordering::SeqCst);
    let _ = write!(stdout, "\r{label}: {completed}/{total_scripts} files");
    let _ = stdout.flush();
}
