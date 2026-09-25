use std::io::Write;
use std::path::Path;

use papyrus_lint_core::content_hash;
use papyrus_lint_live::{config_from_override, lint_source, ParserFailure, ParserFailureKind};

use crate::output::*;

pub(crate) const BLOB_PATH: &str = "<blob>";

/// Lints `source` directly as an in-memory "blob" of Papyrus source text —
/// e.g. a script buffer piped in from an editor or another tool — instead of
/// resolving it from an achlist/`.psc`/directory path on disk, for
/// [`run`]'s `--blob <source>` flag. There's no real file backing `source`,
/// so none of the project-level machinery a normal run needs applies: no
/// project root discovery, no cross-script argument/return type resolution
/// (a call into another script is treated the same as one into an unknown
/// script, since [`papyrus_lints::lint`] never resolves external
/// signatures), no `conflicting_script_versions`/`stale_compiled_output`/
/// `script_filename_mismatch` project lints, and no `compile_check`. The
/// reported path is the literal string `<blob>`, since there's no real path
/// to display.
///
/// `config_path` mirrors `--config <path>`: given, lint configuration is
/// loaded from that file; omitted, the engine's default configuration is
/// used, since there's no project root to discover a `papyrus-lint.yaml`/
/// `.yml` from. `tag_filter` (already validated/normalized via
/// [`normalize_tag_filter`]) restricts the reported diagnostics to one
/// tagged kind, the same as a normal run's `--tag`. `stdout_is_terminal`
/// feeds `--color auto`'s terminal detection the same way [`run`] itself
/// does.
///
/// Returns `0` if no diagnostic counted as a failure (per
/// `fail_on_warning`/`fail_on_info`) and the blob lexed and parsed, `1` if
/// any diagnostic failed the threshold or the blob failed to lex/parse, or
/// `2` on a `--config` load failure or a failure to write `--output <path>`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_blob(
    source: &str,
    config_path: Option<&Path>,
    output_format: OutputFormat,
    hash_source: bool,
    quiet_warnings: bool,
    quiet_info: bool,
    tag_filter: Option<&str>,
    color_choice: ColorChoice,
    output_path: Option<&Path>,
    stdout_is_terminal: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let lint_config = match config_from_override(config_path) {
        Ok(config) => config,
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to load lint config: {err}");
            return 2;
        }
    };

    let analysis = lint_source(source, &lint_config);
    let parse_failed = analysis.parse_failed();
    let mut diagnostics = analysis.diagnostics;
    let parser_errors = blob_parser_errors(analysis.parser_failure);
    let should_fail = finalize_diagnostics(
        &mut diagnostics,
        &lint_config,
        tag_filter,
        quiet_warnings,
        quiet_info,
    );

    let use_color = resolve_color(color_choice, output_path, stdout_is_terminal);

    let total_diagnostics = diagnostics.len();
    let json_diagnostics = to_json_diagnostics(&diagnostics, false);

    let mut report_buf: Vec<u8> = Vec::new();

    match output_format {
        OutputFormat::Json => write_blob_json(
            &mut report_buf,
            json_diagnostics,
            parser_errors,
            total_diagnostics,
            parse_failed,
            should_fail,
        ),
        OutputFormat::Ai => write_blob_ai(
            &mut report_buf,
            &lint_config,
            json_diagnostics,
            parser_errors,
            source,
            hash_source,
            total_diagnostics,
        ),
        OutputFormat::Plain => write_blob_plain(
            &mut report_buf,
            &diagnostics,
            &parser_errors,
            total_diagnostics,
            should_fail || parse_failed,
            use_color,
        ),
    }

    let write_status = flush_report(&report_buf, output_path, stdout, stderr);
    if write_status != 0 {
        return write_status;
    }

    if should_fail || parse_failed {
        1
    } else {
        0
    }
}

fn write_blob_json(
    report_buf: &mut Vec<u8>,
    json_diagnostics: Vec<JsonDiagnostic>,
    parser_errors: Vec<JsonParserError>,
    total_diagnostics: usize,
    parse_failed: bool,
    should_fail: bool,
) {
    let report = JsonReport {
        files: vec![JsonFileReport {
            path: BLOB_PATH.to_string(),
            diagnostics: json_diagnostics,
            parser_errors,
            diff: None,
        }],
        scripts_checked: 1,
        files_with_diagnostics: if total_diagnostics > 0 { 1 } else { 0 },
        total_diagnostics,
        files_fixed: None,
        dry_run: false,
        success: !parse_failed && !should_fail,
    };
    write_json_report(report_buf, &report);
}

fn write_blob_ai(
    report_buf: &mut Vec<u8>,
    lint_config: &papyrus_lints::Config,
    json_diagnostics: Vec<JsonDiagnostic>,
    parser_errors: Vec<JsonParserError>,
    source: &str,
    hash_source: bool,
    total_diagnostics: usize,
) {
    let ai_files = if json_diagnostics.is_empty() && parser_errors.is_empty() {
        Vec::new()
    } else {
        let rule_counts = rule_counts(&json_diagnostics);
        let severity_counts = severity_counts(&json_diagnostics);
        let ai_source = if hash_source {
            AiSource::Hash {
                algorithm: "md5".to_string(),
                hash: content_hash::md5_hex(source),
            }
        } else {
            AiSource::Content {
                content: source.to_string(),
            }
        };
        vec![AiFileReport {
            path: BLOB_PATH.to_string(),
            severity_counts,
            rule_counts,
            diagnostics: json_diagnostics,
            parser_errors,
            source: Some(ai_source),
        }]
    };
    let report = build_ai_report(
        lint_config,
        crate::VERSION,
        ai_files,
        total_diagnostics,
        generated_at(),
    );
    write_json_report(report_buf, &report);
}

fn write_blob_plain(
    report_buf: &mut Vec<u8>,
    diagnostics: &[papyrus_lints::Diagnostic],
    parser_errors: &[JsonParserError],
    total_diagnostics: usize,
    should_fail: bool,
    use_color: bool,
) {
    for error in parser_errors {
        let _ = writeln!(
            report_buf,
            "{}",
            format_parser_error_line(BLOB_PATH, error, use_color)
        );
    }
    for diagnostic in diagnostics {
        let _ = writeln!(
            report_buf,
            "{}",
            format_diagnostic_line(BLOB_PATH, diagnostic, use_color)
        );
    }
    let problem_count = total_diagnostics + parser_errors.len();
    let summary_color = if problem_count == 0 {
        ANSI_GREEN
    } else if should_fail {
        ANSI_RED
    } else {
        ANSI_YELLOW
    };
    let summary = if problem_count == 0 {
        "PapyrusLinterCLI: no problems found in the given blob.".to_string()
    } else {
        format!("PapyrusLinterCLI: {problem_count} problem(s) found in the given blob.")
    };
    let _ = writeln!(
        report_buf,
        "{}",
        colorize(&summary, summary_color, use_color)
    );
}

fn blob_parser_errors(failure: Option<ParserFailure>) -> Vec<JsonParserError> {
    let Some(failure) = failure else {
        return Vec::new();
    };
    vec![JsonParserError {
        kind: match failure.kind {
            ParserFailureKind::Lex => ParserErrorKind::Lex,
            ParserFailureKind::Parse => ParserErrorKind::Parse,
        },
        line: failure.line,
        column: failure.column,
        message: failure.message,
    }]
}

#[cfg(test)]
#[path = "blob_tests.rs"]
mod tests;
