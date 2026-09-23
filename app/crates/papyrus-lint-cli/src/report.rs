//! Folds every resolved script's [`crate::output::FileOutcome`] into one
//! multi-format (plain/JSON/AI) report and flushes it — the aggregation
//! step [`crate::run_lint_command::run_lint_command`] runs once every
//! script has been processed.

use std::io::Write;
use std::path::Path;

use crate::output::*;

#[derive(Default)]
struct AggregatedReport {
    buf: Vec<u8>,
    json_files: Vec<JsonFileReport>,
    ai_files: Vec<AiFileReport>,
    parse_failed: bool,
    should_fail: bool,
    files_with_diagnostics: usize,
    total_diagnostics: usize,
    files_fixed: usize,
}

impl AggregatedReport {
    fn fold(&mut self, outcome: FileOutcome) {
        self.buf.extend_from_slice(&outcome.plain_text);
        if let Some(json_file) = outcome.json_file {
            self.json_files.push(json_file);
        }
        if let Some(ai_file) = outcome.ai_file {
            self.ai_files.push(ai_file);
        }
        self.parse_failed = self.parse_failed || outcome.parse_failed;
        self.should_fail = self.should_fail || outcome.should_fail;
        if outcome.has_diagnostics {
            self.files_with_diagnostics += 1;
            self.total_diagnostics += outcome.diagnostic_count;
        }
        if outcome.fixed {
            self.files_fixed += 1;
        }
    }
}

/// Folds every script's [`FileOutcome`] (already checked for errors by the
/// caller) into one report matching `output_format`, appends its summary
/// line/object, and writes it to `output_path` or `stdout`. Returns the
/// process exit code: `1` if any diagnostic crossed the configured failure
/// threshold or any script failed to lex/parse, `2` on an `--output` write
/// failure, `0` otherwise.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fold_and_flush_report(
    file_results: Vec<Result<FileOutcome, String>>,
    output_format: OutputFormat,
    lint_config: &papyrus_lints::Config,
    scripts_checked: usize,
    fix: bool,
    dry_run: bool,
    use_color: bool,
    progress: bool,
    output_path: Option<&Path>,
    stdout: &mut dyn Write,
    stderr: &mut impl Write,
) -> u8 {
    let mut report = AggregatedReport::default();
    for result in file_results {
        report.fold(result.expect("checked for errors above"));
    }
    if progress {
        let _ = writeln!(stdout);
    }

    append_lint_summary(
        &mut report,
        output_format,
        lint_config,
        scripts_checked,
        fix,
        dry_run,
        use_color,
    );

    let write_status = flush_report(&report.buf, output_path, stdout, stderr);
    if write_status != 0 {
        return write_status;
    }

    if report.should_fail || report.parse_failed {
        1
    } else {
        0
    }
}

fn append_lint_summary(
    report: &mut AggregatedReport,
    output_format: OutputFormat,
    lint_config: &papyrus_lints::Config,
    scripts_checked: usize,
    fix: bool,
    dry_run: bool,
    use_color: bool,
) {
    match output_format {
        OutputFormat::Json => append_json_summary(report, scripts_checked, fix, dry_run),
        OutputFormat::Ai => append_ai_summary(report, lint_config),
        OutputFormat::Plain => {
            append_plain_summary(report, scripts_checked, fix, dry_run, use_color)
        }
    }
}

fn append_json_summary(
    report: &mut AggregatedReport,
    scripts_checked: usize,
    fix: bool,
    dry_run: bool,
) {
    let json_report = JsonReport {
        files: std::mem::take(&mut report.json_files),
        scripts_checked,
        files_with_diagnostics: report.files_with_diagnostics,
        total_diagnostics: report.total_diagnostics,
        files_fixed: fix.then_some(report.files_fixed),
        dry_run,
        success: !report.parse_failed && !report.should_fail,
    };
    write_json_report(&mut report.buf, &json_report);
}

fn append_ai_summary(report: &mut AggregatedReport, lint_config: &papyrus_lints::Config) {
    let ai_report = build_ai_report(
        lint_config,
        crate::VERSION,
        std::mem::take(&mut report.ai_files),
        report.total_diagnostics,
        generated_at(),
    );
    write_json_report(&mut report.buf, &ai_report);
}

fn append_plain_summary(
    report: &mut AggregatedReport,
    scripts_checked: usize,
    fix: bool,
    dry_run: bool,
    use_color: bool,
) {
    let fixed_suffix = if fix && dry_run {
        format!(" ({} script(s) would be fixed.)", report.files_fixed)
    } else if fix {
        format!(" ({} script(s) fixed.)", report.files_fixed)
    } else {
        String::new()
    };

    // Green when clean, yellow when problems were found but none crossed
    // the configured failure threshold, red when the run will exit 1
    // (a lint that fails the threshold, or a lex/parse error).
    let summary_color = if report.total_diagnostics == 0 && !report.parse_failed {
        ANSI_GREEN
    } else if !report.should_fail && !report.parse_failed {
        ANSI_YELLOW
    } else {
        ANSI_RED
    };

    let summary = if report.total_diagnostics == 0 && !report.parse_failed {
        format!("PapyrusLinterCLI: no problems found in {scripts_checked} script(s).{fixed_suffix}")
    } else if report.total_diagnostics == 0 && report.parse_failed {
        format!(
            "PapyrusLinterCLI: parser/lexer error(s) found in {scripts_checked} script(s).{fixed_suffix}"
        )
    } else {
        format!(
            "PapyrusLinterCLI: {} problem(s) found in {} of {scripts_checked} script(s).{fixed_suffix}",
            report.total_diagnostics, report.files_with_diagnostics
        )
    };
    let _ = writeln!(
        report.buf,
        "{}",
        colorize(&summary, summary_color, use_color)
    );
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
