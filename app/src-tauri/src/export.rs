//! Tauri commands backing the desktop app's own "Export issues"/"Export for
//! AI" buttons (see `formatIssuesAsText`/`formatIssuesAsJson`/
//! `formatIssuesForAi` in `app/src/results-export-*.ts`), built on the same
//! `papyrus-lint-output` crate the CLI's `--json`/`--format ai` flags use,
//! so a diagnostic's exported shape can never drift between the GUI and
//! the CLI. Only the shape shared by both is formatted here: the desktop
//! app's own extras (currently active filters, compiler-diagnostic
//! tagging, and per-diagnostic repair previews) are layered on top of this
//! command's output by the frontend, since those need data (the current
//! filter state, an async repair preview per finding) this command has no
//! way to obtain on its own.

use papyrus_lint_output::{
    build_ai_report, format_diagnostic_line, generated_at, rule_counts, severity_counts,
    to_json_diagnostics, AiFileReport, AiSource, JsonFileReport, OwnedDiagnostic,
};
use serde::{Deserialize, Serialize};

/// One file's worth of already-linted, already-filtered findings, as the
/// frontend's `FilteredIssuesFile` sends them for formatting.
#[derive(Debug, Deserialize)]
pub(crate) struct IssuesFileInput {
    path: String,
    findings: Vec<OwnedDiagnostic>,
}

/// Renders `files` the same way the CLI's plain-text report does, one
/// finding per line, so the exported text stays familiar to anyone who's
/// already used the CLI's output. Never colorized, unlike the CLI's own
/// terminal output.
#[tauri::command(async)]
pub(crate) fn format_issues_as_text(files: Vec<IssuesFileInput>) -> String {
    let mut lines = Vec::new();
    for file in &files {
        for finding in &file.findings {
            lines.push(format_diagnostic_line(&file.path, finding, false));
        }
    }
    lines.join("\n")
}

/// The desktop app's own `--json`-shaped export report: a subset of the
/// CLI's own [`papyrus_lint_output::JsonReport`] restricted to what an
/// already-filtered, one-off export has to report (no `scripts_checked`,
/// `files_fixed`, `dry_run`, or `success` -- none of which mean anything
/// once the CLI's own full-project-run/`fix` context is gone).
#[derive(Debug, Serialize)]
pub(crate) struct IssuesJsonReport {
    files: Vec<JsonFileReport>,
    files_with_diagnostics: usize,
    total_diagnostics: usize,
}

/// Renders `files` as JSON, mirroring the shape of the CLI's own `--json`
/// report so both can be consumed by the same tooling. Returns the
/// pretty-printed JSON text directly (rather than a `serde_json::Value`
/// Tauri would re-serialize), since the frontend hands this straight to
/// its "Save As" download helper as a file's contents.
#[tauri::command(async)]
pub(crate) fn format_issues_as_json(files: Vec<IssuesFileInput>) -> String {
    let mut total_diagnostics = 0;
    let json_files: Vec<JsonFileReport> = files
        .into_iter()
        .map(|file| {
            total_diagnostics += file.findings.len();
            JsonFileReport {
                path: file.path,
                diagnostics: to_json_diagnostics(&file.findings, false),
                parser_errors: Vec::new(),
                diff: None,
            }
        })
        .collect();
    let report = IssuesJsonReport {
        files_with_diagnostics: json_files.len(),
        total_diagnostics,
        files: json_files,
    };
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

/// One file's worth of findings for the "Export for AI" document, alongside
/// whatever source the frontend already resolved for it (see
/// `readIssueFileSources` in `app/src/results-export-ai.ts`) -- `None` when
/// nothing was attached at all (distinct from [`AiSource::Error`], which
/// means a resolution was attempted and failed).
#[derive(Debug, Deserialize)]
pub(crate) struct AiIssuesFileInput {
    path: String,
    findings: Vec<OwnedDiagnostic>,
    source: Option<AiSource>,
}

/// Builds the shared base of the desktop app's "Export for AI" document --
/// the header, resolved configuration, per-file/per-report diagnostic
/// counts, and triggered-rule details -- via the same
/// [`papyrus_lint_output::build_ai_report`] the CLI's `--format ai` uses.
/// The frontend (`formatIssuesForAi` in `app/src/results-export-ai.ts`)
/// parses this back, adds its own `filters` field and per-diagnostic
/// `external`/`repair` decorations that need data only it has, and
/// re-serializes the result.
///
/// `files` must already be sorted by (line, column) within each file (see
/// `sortedByPosition`), since the frontend correlates this command's own
/// per-diagnostic order back to its live repair-preview lookups by
/// position rather than by round-tripping an id.
#[tauri::command(async)]
pub(crate) fn format_issues_for_ai_base(
    files: Vec<AiIssuesFileInput>,
    configuration: papyrus_lints::Config,
    version: String,
) -> String {
    let ai_files: Vec<AiFileReport> = files
        .into_iter()
        .map(|file| {
            let diagnostics = to_json_diagnostics(&file.findings, true);
            AiFileReport {
                severity_counts: severity_counts(&diagnostics),
                rule_counts: rule_counts(&diagnostics),
                diagnostics,
                parser_errors: Vec::new(),
                path: file.path,
                source: file.source,
            }
        })
        .collect();
    let total_diagnostics = ai_files.iter().map(|file| file.diagnostics.len()).sum();

    let report = build_ai_report(
        &configuration,
        &version,
        ai_files,
        total_diagnostics,
        generated_at(),
    );
    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
