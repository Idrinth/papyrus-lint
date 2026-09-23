// Plain-text/JSON/AI-export report *formatting* now lives in the shared
// `papyrus-lint-output` crate (see its own docs), used here and by the
// desktop app's Tauri commands (`app/src-tauri/src/export.rs`) so the two
// can never disagree on a diagnostic's exported shape. This module keeps
// only what's specific to running the CLI itself: which format was
// selected, filtering/failure-threshold logic driven by CLI flags, and
// writing the finished report to stdout or `--output <path>`.
pub(crate) use papyrus_lint_output::{
    build_ai_report, colorize, format_diagnostic_line, format_parser_error_line, generated_at,
    resolve_color, rule_counts, severity_counts, to_json_diagnostics, AiFileReport, AiSource,
    ColorChoice, ParserErrorKind, ANSI_GREEN, ANSI_RED, ANSI_YELLOW,
};
pub use papyrus_lint_output::{JsonDiagnostic, JsonFileReport, JsonParserError, JsonReport};

/// Collects the lexer/parser errors raised while handling `source`. Empty
/// when the script lexes and parses cleanly. Currently at most one entry
/// because [`papyrus_parser::parse`] stops at the first error.
pub(crate) fn collect_parser_errors(source: &str) -> Vec<JsonParserError> {
    match papyrus_parser::parse(source) {
        Ok(_) => Vec::new(),
        Err(papyrus_parser::PapyrusError::Lex(error)) => vec![JsonParserError {
            kind: ParserErrorKind::Lex,
            line: error.line,
            column: error.col,
            message: error.message,
        }],
        Err(papyrus_parser::PapyrusError::Parse(error)) => vec![JsonParserError {
            kind: ParserErrorKind::Parse,
            line: error.line,
            column: error.col,
            message: error.message,
        }],
    }
}

/// Normalizes a raw `--tag <kind>` value to lowercase and checks it against
/// every rule's own tagged kind(s) (see [`papyrus_lints::tags::RULE_TAGS`]),
/// matched case-insensitively. Returns `Ok(None)` for no `--tag` at all,
/// `Ok(Some(normalized))` for a recognized kind, or `Err(value)` (the
/// original, un-normalized value, for the caller's own error message) for
/// one that matches no rule's kind. Shared by the normal lint/fix path and
/// [`crate::run_blob`], so both report the exact same "unknown tag" error.
pub(crate) fn normalize_tag_filter(tag_filter: Option<String>) -> Result<Option<String>, String> {
    match tag_filter {
        Some(value) => {
            let normalized = value.to_ascii_lowercase();
            let known = papyrus_lints::tags::RULE_TAGS.iter().any(|rule_tags| {
                rule_tags
                    .kinds
                    .iter()
                    .any(|kind| kind.eq_ignore_ascii_case(&normalized))
            });
            if known {
                Ok(Some(normalized))
            } else {
                Err(value)
            }
        }
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OutputFormat {
    Plain,
    Json,
    Ai,
}

/// Finalizes one script's (or the `--blob` source's) diagnostics: applies
/// `--tag` filtering, sorts by location, determines whether any diagnostic
/// -- even one about to be hidden below -- crosses the configured
/// `fail_on_warning`/`fail_on_info` threshold, then applies
/// `--quiet-warnings`/`--quiet-info`. Returns that failure verdict, computed
/// before the quiet flags hide anything, so a hidden diagnostic still
/// affects the exit code the same as a shown one. Shared by the per-file
/// lint pass in [`crate::run`] and [`crate::run_blob`] so the two can't
/// drift on what counts as a failure versus what's merely hidden from the
/// report.
pub(crate) fn finalize_diagnostics(
    diagnostics: &mut Vec<papyrus_lints::Diagnostic>,
    lint_config: &papyrus_lints::Config,
    tag_filter: Option<&str>,
    quiet_warnings: bool,
    quiet_info: bool,
) -> bool {
    if let Some(tag) = tag_filter {
        diagnostics.retain(|diagnostic| {
            papyrus_lints::tags::tags_for(diagnostic.rule).is_some_and(|rule_tags| {
                rule_tags
                    .kinds
                    .iter()
                    .any(|kind| kind.eq_ignore_ascii_case(tag))
            })
        });
    }
    diagnostics.sort_by_key(|d| (d.line, d.column));

    let should_fail = diagnostics
        .iter()
        .any(|diagnostic| lint_config.should_fail_on(diagnostic));
    diagnostics.retain(|diagnostic| {
        !((quiet_warnings && diagnostic.level() == "warning")
            || (quiet_info && diagnostic.level() == "info"))
    });
    should_fail
}

/// Serializes `report` as pretty-printed JSON, followed by a trailing
/// newline, into `buf` -- the shared tail of every `--json`/`--format ai`
/// branch in [`crate::run`] and [`crate::run_blob`].
pub(crate) fn write_json_report(buf: &mut Vec<u8>, report: &impl serde::Serialize) {
    use std::io::Write;
    let _ = writeln!(
        buf,
        "{}",
        serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
    );
}

/// Writes `report_buf` to `--output <path>` when one was given, or to
/// `stdout` otherwise. Shared by [`crate::run`]'s lint/fix path and
/// [`crate::run_blob`] so a failure to create the report file is reported
/// with the same `error: failed to write ...` message in both.
pub(crate) fn flush_report(
    report_buf: &[u8],
    output_path: Option<&std::path::Path>,
    stdout: &mut dyn std::io::Write,
    stderr: &mut dyn std::io::Write,
) -> u8 {
    if let Some(output_path) = output_path {
        if let Err(err) = std::fs::write(output_path, report_buf) {
            let _ = writeln!(
                stderr,
                "error: failed to write {}: {err}",
                output_path.display()
            );
            return 2;
        }
    } else {
        let _ = stdout.write_all(report_buf);
    }
    0
}

/// One script's worth of work from the parallel lint loop in [`run`],
/// collected by its worker so the main thread can fold it into the overall
/// report afterward in the script's original (not completion) order --
/// see [`papyrus_lint_core::parallel::map_in_parallel`].
pub(crate) struct FileOutcome {
    /// This file's own slice of the plain-text report (a dry-run diff, if
    /// any, followed by its diagnostic lines), empty in JSON/AI mode.
    pub(crate) plain_text: Vec<u8>,
    pub(crate) json_file: Option<JsonFileReport>,
    pub(crate) ai_file: Option<AiFileReport>,
    /// Whether this file's final source failed to parse. Kept separate from
    /// `should_fail` because it affects JSON's `success` field, not the
    /// established CLI exit status.
    pub(crate) parse_failed: bool,
    /// Whether any of this file's diagnostics (even one hidden by
    /// `--quiet-warnings`/`--quiet-info`) crosses the configured
    /// `fail_on_warning`/`fail_on_info` threshold.
    pub(crate) should_fail: bool,
    /// Whether this file has at least one diagnostic left after quiet
    /// filtering, i.e. one that's actually reported.
    pub(crate) has_diagnostics: bool,
    /// How many diagnostics are left after quiet filtering.
    pub(crate) diagnostic_count: usize,
    /// Whether `fix` actually changed this file (or, under `--dry-run`,
    /// would have).
    pub(crate) fixed: bool,
}

#[cfg(test)]
mod tests;
