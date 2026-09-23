use serde::Serialize;

use crate::diagnostic::{level_of, strip_severity_prefix, DiagnosticLike};

/// A single diagnostic as printed by the CLI's `--json`/`--format ai`, or
/// returned by the desktop app's own `format_issues_as_json`/
/// `format_issues_for_ai_base` Tauri commands: mirrors the plain-text
/// `<path>:<line>:<column>: [<rule>] <message>` line but with `level` (see
/// [`level_of`]) broken out as its own field rather than left for a
/// consumer to parse back out of `message`.
#[derive(Debug, Serialize)]
pub struct JsonDiagnostic {
    pub line: usize,
    pub column: usize,
    pub rule: String,
    pub level: &'static str,
    pub message: String,
    /// This rule's own documentation link (`RuleTags::doc_url`), so a
    /// consumer (an editor extension, the SublimeLinter plugin, the
    /// desktop app's "Export for AI" document) can jump a user straight to
    /// it instead of just showing the rule id. `None` for a rule with no
    /// [`papyrus_lints::tags`] metadata (e.g. a compiler-reported
    /// diagnostic).
    pub doc_url: Option<String>,
}

/// Looks up `rule`'s [`papyrus_lints::tags::RuleTags::doc_url`], for
/// building a [`JsonDiagnostic`]/`AiRuleDetails`.
pub fn doc_url_for(rule: &str) -> Option<String> {
    papyrus_lints::tags::tags_for(rule).map(|tags| tags.doc_url())
}

/// Converts already-finalized diagnostics into their `--json`/`--format
/// ai`/`format_issues_as_json`/`format_issues_for_ai_base` shape. Shared by
/// every one of those so they never disagree on how a diagnostic maps onto
/// the reported JSON fields.
///
/// `strip_severity_prefix` drops the leading `[error]`/`[warning]`/`[info]`
/// tag from `message` (see [`crate::strip_severity_prefix`]), since `level`
/// already carries it as its own field; the CLI's `--json`/`--format ai`
/// keep the tag in `message` (`strip_severity_prefix: false`), while the
/// desktop app's "Export for AI" document strips it (`true`).
pub fn to_json_diagnostics<D: DiagnosticLike>(
    diagnostics: &[D],
    strip_severity_prefix_from_message: bool,
) -> Vec<JsonDiagnostic> {
    diagnostics
        .iter()
        .map(|d| {
            let message = if strip_severity_prefix_from_message {
                strip_severity_prefix(d.message())
            } else {
                d.message().to_string()
            };
            JsonDiagnostic {
                line: d.line(),
                column: d.column(),
                rule: d.rule().to_string(),
                level: level_of(d.message()),
                message,
                doc_url: doc_url_for(d.rule()),
            }
        })
        .collect()
}

/// One resolved script's diagnostics, as printed by the CLI's `--json` or
/// returned by the desktop app's `format_issues_as_json` command.
#[derive(Debug, Serialize)]
pub struct JsonFileReport {
    pub path: String,
    pub diagnostics: Vec<JsonDiagnostic>,
    /// The standard unified diff between this script's original source and
    /// what `fix` would have written, only non-`null` under the CLI's `fix
    /// --dry-run`. Always `None` from the desktop app's own export
    /// commands, which have nothing to do with `fix`.
    pub diff: Option<String>,
}

/// The full report printed to stdout by the CLI's `--json`, in place of the
/// plain-text diagnostics lines and summary.
#[derive(Debug, Serialize)]
pub struct JsonReport {
    pub files: Vec<JsonFileReport>,
    pub scripts_checked: usize,
    pub files_with_diagnostics: usize,
    pub total_diagnostics: usize,
    /// Only present when run with the `fix` subcommand. Under `--dry-run`,
    /// counts scripts that *would* have been fixed rather than scripts
    /// actually rewritten on disk.
    pub files_fixed: Option<usize>,
    /// Whether this run was `fix --dry-run`: no file was written, and each
    /// changed script's [`JsonFileReport::diff`] instead shows what would
    /// have changed. Always `false` outside `fix --dry-run`.
    pub dry_run: bool,
    /// Whether the run would exit `0`: every script parsed and no
    /// diagnostics counted as a failure per `fail_on_warning`/`fail_on_info`
    /// (see [`papyrus_lints::Config::should_fail_on`]).
    pub success: bool,
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
