use serde::Serialize;

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

/// A single diagnostic as printed by `--json`, mirroring the plain-text
/// `<path>:<line>:<column>: [<rule>] <message>` line but with `level`
/// (see [`papyrus_lints::Diagnostic::level`]) broken out as its own field
/// rather than left for a consumer to parse back out of `message`.
#[derive(Debug, Serialize)]
pub struct JsonDiagnostic {
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub level: &'static str,
    pub message: String,
    /// This rule's own documentation link (`RuleTags::doc_url`), so a
    /// consumer (an editor extension, the SublimeLinter plugin) can jump a
    /// user straight to it instead of just showing the rule id. `None` for
    /// a rule with no [`papyrus_lints::tags`] metadata (e.g. a
    /// compiler-reported diagnostic — see
    /// `papyrus_lint_core::compile_diagnostics`).
    pub doc_url: Option<String>,
}

/// Looks up `rule`'s [`papyrus_lints::tags::RuleTags::doc_url`], for
/// building a [`JsonDiagnostic`]/`AiRuleDetails`.
pub(crate) fn doc_url_for(rule: &str) -> Option<String> {
    papyrus_lints::tags::tags_for(rule).map(|tags| tags.doc_url())
}

/// One resolved script's diagnostics, as printed by `--json`. Every
/// resolved script gets an entry, even one with no diagnostics, so a
/// consumer (e.g. an editor plugin) can clear stale diagnostics for a
/// file that's since become clean.
#[derive(Debug, Serialize)]
pub struct JsonFileReport {
    pub path: String,
    pub diagnostics: Vec<JsonDiagnostic>,
    /// The standard unified diff between this script's original source and
    /// what `fix` would have written, only non-`null` when run with `fix
    /// --dry-run` and this script would actually have changed.
    pub diff: Option<String>,
}

/// The full report printed to stdout by `--json`, in place of the
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
    /// Whether the run would exit `0`: no diagnostics counted as a
    /// failure per `fail_on_warning`/`fail_on_info` (see
    /// [`papyrus_lints::Config::should_fail_on`]).
    pub success: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiHeader {
    pub(crate) tool: &'static str,
    pub(crate) version: &'static str,
    pub(crate) website: &'static str,
    pub(crate) target_game: &'static str,
    pub(crate) generated_at: String,
}

/// How an [`AiFileReport`]'s source is represented: the CLI always has a
/// script's contents in hand by the time it builds one (a read failure
/// aborts the whole run earlier instead), so unlike the desktop app's own
/// "Export for AI" feature (see `formatIssuesForAi` in `app/src/main.ts`,
/// whose `source` field also covers a read-error or "nothing attached"
/// case), the CLI only ever emits one of these two shapes.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AiSource {
    /// The script's full on-disk contents.
    Content { content: String },
    /// The script's content hash instead of its full text, selected via
    /// `--hash-source` so a report can be handed to an AI without exposing
    /// proprietary script text; a viewer can still tell files apart, or
    /// notice a file changed between exports, from the hash alone.
    Hash {
        algorithm: &'static str,
        hash: String,
    },
}

#[derive(Debug, Serialize)]
pub(crate) struct AiFileReport {
    pub(crate) path: String,
    pub(crate) severity_counts: AiSeverityCounts,
    pub(crate) rule_counts: std::collections::BTreeMap<&'static str, usize>,
    pub(crate) diagnostics: Vec<JsonDiagnostic>,
    pub(crate) source: AiSource,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiFindings {
    pub(crate) files: Vec<AiFileReport>,
    pub(crate) total_diagnostics: usize,
    pub(crate) severity_counts: AiSeverityCounts,
    pub(crate) rule_counts: std::collections::BTreeMap<&'static str, usize>,
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct AiSeverityCounts {
    pub(crate) errors: usize,
    pub(crate) warnings: usize,
    pub(crate) info: usize,
}

pub(crate) fn severity_counts<'a>(
    diagnostics: impl IntoIterator<Item = &'a JsonDiagnostic>,
) -> AiSeverityCounts {
    let mut counts = AiSeverityCounts::default();
    for diagnostic in diagnostics {
        match diagnostic.level {
            "error" => counts.errors += 1,
            "warning" => counts.warnings += 1,
            "info" => counts.info += 1,
            _ => {}
        }
    }
    counts
}

pub(crate) fn rule_counts<'a>(
    diagnostics: impl IntoIterator<Item = &'a JsonDiagnostic>,
) -> std::collections::BTreeMap<&'static str, usize> {
    let mut counts = std::collections::BTreeMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.rule).or_insert(0) += 1;
    }
    counts
}

#[derive(Debug, Serialize)]
pub(crate) struct AiRuleDetails {
    pub(crate) rule: &'static str,
    pub(crate) description: &'static str,
    pub(crate) kinds: &'static [&'static str],
    pub(crate) importance: papyrus_lints::tags::Importance,
    pub(crate) auto_fixable: bool,
    /// This rule's own documentation link (`RuleTags::doc_url`), so the
    /// assistant reading the export can look up its full explanation on the
    /// project website.
    pub(crate) doc_url: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AiReport {
    #[serde(rename = "$schema")]
    pub(crate) schema: &'static str,
    pub(crate) header: AiHeader,
    pub(crate) configuration: serde_json::Value,
    pub(crate) findings: AiFindings,
    pub(crate) rule_details: Vec<AiRuleDetails>,
}

/// The AI export's own `configuration` shape: the same resolved
/// [`papyrus_lints::Config`] a lint run used, except its `rules` object
/// (58 individual enable flags, each with its own description in the
/// schema) is replaced with a compact, alphabetically sorted
/// `enabled_rules` list of just the ids that are currently on (see
/// [`papyrus_lints::config::Rules::enabled_ids`]) - no information is
/// lost, since a rule absent from the list is simply disabled, but every
/// export no longer repeats a large, mostly-constant block of booleans.
/// Mirrors `aiConfiguration` in `app/src/main.ts`.
pub(crate) fn ai_configuration(config: &papyrus_lints::Config) -> serde_json::Value {
    let mut value = serde_json::to_value(config).expect("Config always serializes to an object");
    let object = value
        .as_object_mut()
        .expect("Config serializes as a JSON object");
    object.remove("rules");
    object.insert(
        "enabled_rules".to_string(),
        serde_json::Value::from(config.rules.enabled_ids()),
    );
    value
}

/// Returns the current UTC time in the millisecond-precision RFC 3339 form
/// also produced by JavaScript's `Date.toISOString()` in the frontend.
pub(crate) fn generated_at() -> String {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format_unix_timestamp(elapsed.as_secs(), elapsed.subsec_millis())
}

pub(crate) fn format_unix_timestamp(seconds: u64, milliseconds: u32) -> String {
    let days = (seconds / 86_400) as i64;
    let seconds_in_day = seconds % 86_400;
    // Gregorian civil-from-days conversion (the epoch offset makes day zero
    // 1970-01-01).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let hour = seconds_in_day / 3_600;
    let minute = seconds_in_day % 3_600 / 60;
    let second = seconds_in_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum OutputFormat {
    Plain,
    Json,
    Ai,
}

pub(crate) const ANSI_RESET: &str = "\x1b[0m";
pub(crate) const ANSI_BOLD: &str = "\x1b[1m";
pub(crate) const ANSI_DIM: &str = "\x1b[2m";
pub(crate) const ANSI_RED: &str = "\x1b[31m";
pub(crate) const ANSI_YELLOW: &str = "\x1b[33m";
pub(crate) const ANSI_CYAN: &str = "\x1b[36m";
pub(crate) const ANSI_GREEN: &str = "\x1b[32m";

/// Whether/when to colorize the plain-text report, set by the `--color`
/// flag (see [`USAGE`]). Never affects `--json` output, which is meant for
/// tooling rather than a terminal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// Wraps `text` in `code`/reset ANSI escapes when `use_color` is true,
/// otherwise returns it unchanged.
pub(crate) fn colorize(text: &str, code: &str, use_color: bool) -> String {
    if use_color {
        format!("{code}{text}{ANSI_RESET}")
    } else {
        text.to_string()
    }
}

pub(crate) fn level_color(level: &str) -> &'static str {
    match level {
        "error" => ANSI_RED,
        "warning" => ANSI_YELLOW,
        "info" => ANSI_CYAN,
        _ => ANSI_RESET,
    }
}

/// Renders one diagnostic's plain-text report line (`<path>:<line>:<column>:
/// [<rule>] <message>`), colorizing the location, the rule tag, and the
/// `[error]`/`[warning]`/`[info]` level tag already embedded at the front of
/// `diagnostic.message` (see [`papyrus_lints::Diagnostic::level`]) when
/// `use_color` is true. A rule with known [`papyrus_lints::tags`] metadata
/// (i.e. a real lint rather than e.g. a compiler-reported diagnostic) gets
/// its documentation link (`RuleTags::doc_url`) appended, so a reader can
/// jump straight to that rule's own explanation instead of just seeing its
/// id.
pub(crate) fn format_diagnostic_line(
    path_display: &str,
    diagnostic: &papyrus_lints::Diagnostic,
    use_color: bool,
) -> String {
    let doc_url_suffix = papyrus_lints::tags::tags_for(diagnostic.rule)
        .map(|tags| format!(" ({})", tags.doc_url()))
        .unwrap_or_default();

    if !use_color {
        return format!(
            "{}:{}:{}: [{}] {}{}",
            path_display,
            diagnostic.line,
            diagnostic.column,
            diagnostic.rule,
            diagnostic.message,
            doc_url_suffix
        );
    }

    let level = diagnostic.level();
    let level_tag = format!("[{level}]");
    let message = match diagnostic.message.strip_prefix(level_tag.as_str()) {
        Some(rest) => format!("{}{rest}", colorize(&level_tag, level_color(level), true)),
        None => diagnostic.message.clone(),
    };

    format!(
        "{}: {} {}{}",
        colorize(
            &format!("{path_display}:{}:{}", diagnostic.line, diagnostic.column),
            ANSI_BOLD,
            true
        ),
        colorize(&format!("[{}]", diagnostic.rule), ANSI_DIM, true),
        message,
        if doc_url_suffix.is_empty() {
            String::new()
        } else {
            colorize(&doc_url_suffix, ANSI_DIM, true)
        }
    )
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
mod tests {
    use super::*;
    use crate::test_support::*;
    use papyrus_lint_core::content_hash;
    use std::fs;

    #[test]
    fn unix_timestamps_are_formatted_as_utc_rfc3339() {
        assert_eq!(format_unix_timestamp(0, 0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            format_unix_timestamp(1_709_251_199, 42),
            "2024-02-29T23:59:59.042Z"
        );
    }

    #[test]
    fn unix_timestamps_handle_calendar_boundaries_and_millisecond_padding() {
        assert_eq!(
            format_unix_timestamp(31_535_999, 7),
            "1970-12-31T23:59:59.007Z"
        );
        assert_eq!(
            format_unix_timestamp(31_536_000, 70),
            "1971-01-01T00:00:00.070Z"
        );
        assert_eq!(
            format_unix_timestamp(951_782_400, 999),
            "2000-02-29T00:00:00.999Z"
        );
    }

    #[test]
    fn severity_counts_tallies_each_known_level_and_ignores_unknown_levels() {
        let diagnostics = [
            JsonDiagnostic {
                line: 1,
                column: 1,
                rule: "first-rule",
                level: "error",
                message: "first".to_string(),
                doc_url: None,
            },
            JsonDiagnostic {
                line: 2,
                column: 1,
                rule: "second-rule",
                level: "warning",
                message: "second".to_string(),
                doc_url: None,
            },
            JsonDiagnostic {
                line: 3,
                column: 1,
                rule: "third-rule",
                level: "info",
                message: "third".to_string(),
                doc_url: None,
            },
            JsonDiagnostic {
                line: 4,
                column: 1,
                rule: "future-rule",
                level: "notice",
                message: "future".to_string(),
                doc_url: None,
            },
        ];

        let counts = severity_counts(&diagnostics);

        assert_eq!(counts.errors, 1);
        assert_eq!(counts.warnings, 1);
        assert_eq!(counts.info, 1);
    }

    #[test]
    fn rule_counts_aggregates_duplicates_in_sorted_rule_order() {
        let diagnostics = [
            JsonDiagnostic {
                line: 1,
                column: 1,
                rule: "z-rule",
                level: "warning",
                message: "first".to_string(),
                doc_url: None,
            },
            JsonDiagnostic {
                line: 2,
                column: 1,
                rule: "a-rule",
                level: "warning",
                message: "second".to_string(),
                doc_url: None,
            },
            JsonDiagnostic {
                line: 3,
                column: 1,
                rule: "z-rule",
                level: "warning",
                message: "third".to_string(),
                doc_url: None,
            },
        ];

        let counts = rule_counts(&diagnostics);

        assert_eq!(
            counts.keys().copied().collect::<Vec<_>>(),
            ["a-rule", "z-rule"]
        );
        assert_eq!(counts["a-rule"], 1);
        assert_eq!(counts["z-rule"], 2);
    }

    #[test]
    fn ai_configuration_replaces_rule_flags_with_enabled_rule_ids() {
        let config = papyrus_lints::Config::default();

        let value = ai_configuration(&config);
        let object = value
            .as_object()
            .expect("configuration should be an object");
        let enabled = object["enabled_rules"]
            .as_array()
            .expect("enabled_rules should be an array");

        assert!(!object.contains_key("rules"));
        assert!(enabled.contains(&serde_json::json!("trailing-whitespace")));
        assert!(!enabled.contains(&serde_json::json!("property-sorting")));
    }

    #[test]
    fn json_flag_prints_a_single_json_report_instead_of_plain_text() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert_eq!(stderr, "");
        assert!(!stdout.contains("PapyrusLinterCLI:"));

        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be a single JSON document");
        assert_eq!(report["success"], true);
        assert_eq!(report["scripts_checked"], 1);
        assert_eq!(report["files_with_diagnostics"], 1);
        assert_eq!(report["total_diagnostics"], 1);
        assert!(report["files_fixed"].is_null());
        let files = report["files"]
            .as_array()
            .expect("files should be an array");
        assert_eq!(files.len(), 1);
        let diagnostics = files[0]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["rule"], "trailing-whitespace");
        assert_eq!(diagnostics[0]["level"], "warning");
        assert_eq!(diagnostics[0]["line"], 1);
    }

    #[test]
    fn json_flag_lists_every_resolved_script_including_clean_ones() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["success"], true);
        let files = report["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_flag_combines_with_the_fix_subcommand() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["files_fixed"], 1);
        let files = report["files"].as_array().unwrap();
        let diagnostics = files[0]["diagnostics"].as_array().unwrap();
        assert!(diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("Game.GetPlayer")));
    }

    #[test]
    fn json_flag_combines_with_fix_dry_run() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1, "stderr: {stderr}");
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
            "--dry-run must never write to the file, even combined with --json"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["dry_run"], true);
        assert_eq!(report["files_fixed"], 1);
        let files = report["files"].as_array().unwrap();
        let diff = files[0]["diff"].as_str().expect("diff should be a string");
        assert!(diff.contains("-ScriptName Example   \n"));
        assert!(diff.contains("+ScriptName Example\n"));
    }

    #[test]
    fn json_flag_can_precede_the_fix_subcommand() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "fix".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["files_fixed"], 1);
        assert_eq!(report["total_diagnostics"], 0);
        assert_eq!(report["success"], true);
    }

    #[test]
    fn output_flag_writes_the_plain_text_report_to_a_file_instead_of_stdout() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.txt");

        let (code, stdout, stderr) = run_captured(&[
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.is_empty());
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(contents.contains("[trailing-whitespace]"));
        assert!(contents.contains("1 problem(s) found in 1 of 1 script(s)"));
    }

    #[test]
    fn output_flag_writes_the_json_report_to_a_file_instead_of_stdout() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.json");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.is_empty());
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        let report: serde_json::Value =
            serde_json::from_str(&contents).expect("output file should contain a JSON document");
        assert_eq!(report["success"], true);
        assert_eq!(report["total_diagnostics"], 1);
    }

    #[test]
    fn output_flag_combines_with_fix() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.txt");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stdout.is_empty());
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example\n"
        );
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(contents.contains("no problems found in 1 script"));
        assert!(contents.contains("(1 script(s) fixed.)"));
    }

    #[test]
    fn output_flag_errors_when_the_directory_does_not_exist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("missing-dir/report.txt");

        let (code, _stdout, stderr) = run_captured(&[
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.starts_with("error: failed to write"));
    }

    #[test]
    fn output_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--output".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn progress_flag_requires_output_in_plain_text_mode() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "--progress".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.contains("--progress requires --output"));
    }

    #[test]
    fn progress_flag_requires_output_in_json_mode() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "--progress".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.contains("--progress requires --output"));
    }

    #[test]
    fn progress_flag_prints_a_progress_bar_to_stdout_when_output_is_set() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/One.psc"),
            "ScriptName One\n",
        );
        write_file(
            &dir.path().join("scripts/source/Two.psc"),
            "ScriptName Two\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/One.psc", "scripts/source/Two.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.txt");

        let (code, stdout, stderr) = run_captured(&[
            "--progress".to_string(),
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.contains("\rLinting: 1/2 files"));
        assert!(stdout.contains("\rLinting: 2/2 files"));
        assert!(stdout.ends_with('\n'));
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(contents.contains("no problems found in 2 script"));
    }

    #[test]
    fn plain_text_report_is_uncolored_when_stdout_is_not_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) =
            run_captured_with_terminal_stdout(&[script_path.to_string_lossy().into_owned()], false);

        assert_eq!(code, 0);
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(!stdout.contains('\x1b'));
    }

    #[test]
    fn color_auto_colorizes_when_stdout_is_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) =
            run_captured_with_terminal_stdout(&[script_path.to_string_lossy().into_owned()], true);

        assert_eq!(code, 0);
        assert!(stdout.contains('\x1b'));
        // The rule id and level tag both still appear verbatim inside the
        // colorized escapes, so consumers scraping for them (and the other
        // tests here) still find them.
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("[warning]"));
    }

    #[test]
    fn color_never_disables_color_even_on_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) = run_captured_with_terminal_stdout(
            &[
                "--color".to_string(),
                "never".to_string(),
                script_path.to_string_lossy().into_owned(),
            ],
            true,
        );

        assert_eq!(code, 0);
        assert!(!stdout.contains('\x1b'));
    }

    #[test]
    fn color_always_enables_color_even_without_a_terminal() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) = run_captured(&[
            "--color=always".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stdout.contains('\x1b'));
    }

    #[test]
    fn color_auto_does_not_colorize_a_file_written_via_output() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");
        let output_path = dir.path().join("report.txt");

        let (code, _stdout, _stderr) = run_captured_with_terminal_stdout(
            &[
                "--output".to_string(),
                output_path.to_string_lossy().into_owned(),
                script_path.to_string_lossy().into_owned(),
            ],
            true,
        );

        assert_eq!(code, 0);
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(!contents.contains('\x1b'));
    }

    #[test]
    fn color_flag_rejects_an_unknown_value() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--color".to_string(),
            "rainbow".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--color must be"));
    }

    #[test]
    fn color_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--color".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn json_output_is_never_colorized_even_when_color_is_always() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured_with_terminal_stdout(
            &[
                "--json".to_string(),
                "--color=always".to_string(),
                script_path.to_string_lossy().into_owned(),
            ],
            true,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(!stdout.contains('\x1b'));
        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("colored JSON would not parse");
        assert_eq!(report["total_diagnostics"], 1);
    }

    #[test]
    fn diagnostic_formatter_colorizes_each_structural_part() {
        let diagnostic = papyrus_lints::Diagnostic {
            line: 4,
            column: 7,
            rule: "example-rule",
            message: "[warning] example message".to_string(),
        };

        let formatted = format_diagnostic_line("Example.psc", &diagnostic, true);

        assert!(formatted.contains("\x1b[1mExample.psc:4:7\x1b[0m"));
        assert!(formatted.contains("\x1b[2m[example-rule]\x1b[0m"));
        assert!(formatted.contains("\x1b[33m[warning]\x1b[0m example message"));
    }

    #[test]
    fn diagnostic_formatter_preserves_an_untagged_message() {
        let diagnostic = papyrus_lints::Diagnostic {
            line: 1,
            column: 2,
            rule: "example-rule",
            message: "example message without a level tag".to_string(),
        };

        let formatted = format_diagnostic_line("Example.psc", &diagnostic, true);

        assert!(formatted.ends_with("example message without a level tag"));
        assert!(!formatted.contains("\x1b[31m[error]"));
    }

    #[test]
    fn output_replaces_an_existing_report_instead_of_appending() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script = dir.path().join("Example.psc");
        let report = dir.path().join("report.txt");
        write_file(&script, "ScriptName Example\n");
        write_file(&report, "stale report contents that must disappear\n");

        let (code, stdout, stderr) = run_captured(&[
            "--output".to_string(),
            report.to_string_lossy().into_owned(),
            script.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        assert_eq!(
            fs::read_to_string(report).expect("failed to read report"),
            "PapyrusLinterCLI: no problems found in 1 script(s).\n"
        );
    }

    #[test]
    fn ai_format_reports_source_metadata_counts_and_rule_details() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let dirty = dir.path().join("Dirty.psc");
        let clean = dir.path().join("Clean.psc");
        let dirty_source = "ScriptName Dirty   \n";
        write_file(&dirty, dirty_source);
        write_file(&clean, "ScriptName Clean\n");

        let (code, stdout, stderr) = run_captured(&[
            "--format=ai".to_string(),
            dir.path().to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stderr.is_empty());
        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("AI report should be valid JSON");
        assert_eq!(
            report["$schema"],
            "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json"
        );
        assert_eq!(report["header"]["tool"], "Papyrus Lint");
        assert_eq!(report["header"]["target_game"], "Skyrim SE/AE");
        assert_eq!(report["findings"]["total_diagnostics"], 1);
        assert_eq!(report["findings"]["severity_counts"]["warnings"], 1);
        assert_eq!(report["findings"]["rule_counts"]["trailing-whitespace"], 1);
        assert_eq!(report["findings"]["files"].as_array().unwrap().len(), 1);
        assert_eq!(report["findings"]["files"][0]["source"]["type"], "content");
        assert_eq!(
            report["findings"]["files"][0]["source"]["content"],
            dirty_source
        );
        assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
        assert_eq!(report["rule_details"][0]["auto_fixable"], true);
        assert!(report["configuration"]["enabled_rules"].is_array());
        assert!(report["configuration"].get("rules").is_none());
    }

    #[test]
    fn ai_hash_source_replaces_script_contents_with_an_md5_digest() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script = dir.path().join("Example.psc");
        let source = "ScriptName Example   \n";
        write_file(&script, source);

        let (code, stdout, stderr) = run_captured(&[
            "--format".to_string(),
            "ai".to_string(),
            "--hash-source".to_string(),
            script.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("AI report should be valid JSON");
        let source_report = &report["findings"]["files"][0]["source"];
        assert_eq!(source_report["type"], "hash");
        assert_eq!(source_report["algorithm"], "md5");
        assert_eq!(source_report["hash"], content_hash::md5_hex(source));
        assert!(source_report.get("content").is_none());
        assert!(!stdout.contains(source));
    }

    #[test]
    fn hash_source_without_ai_format_is_a_usage_error() {
        let (code, stdout, stderr) =
            run_captured(&["--hash-source".to_string(), "Example.psc".to_string()]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert_eq!(stderr, "error: --hash-source requires --format ai\n");
    }

    #[test]
    fn format_flag_rejects_unknown_values_and_conflicts_with_json() {
        let (unknown_code, unknown_stdout, unknown_stderr) =
            run_captured(&["--format=yaml".to_string(), "Example.psc".to_string()]);
        assert_eq!(unknown_code, 2);
        assert!(unknown_stdout.is_empty());
        assert_eq!(
            unknown_stderr,
            "error: --format must be 'plain', 'json', or 'ai', got 'yaml'\n"
        );

        let (conflict_code, conflict_stdout, conflict_stderr) = run_captured(&[
            "--json".to_string(),
            "--format=json".to_string(),
            "Example.psc".to_string(),
        ]);
        assert_eq!(conflict_code, 2);
        assert!(conflict_stdout.is_empty());
        assert_eq!(
            conflict_stderr,
            "error: --json and --format can't be combined\n"
        );
    }

    #[test]
    fn level_colors_cover_info_and_unknown_diagnostic_levels() {
        assert_eq!(level_color("info"), ANSI_CYAN);
        assert_eq!(level_color("notice"), ANSI_RESET);

        let info = papyrus_lints::Diagnostic {
            line: 2,
            column: 3,
            rule: "example-rule",
            message: "[info] informational diagnostic".to_string(),
        };
        let formatted = format_diagnostic_line("Example.psc", &info, true);

        assert!(formatted.contains(&format!("{ANSI_CYAN}[info]{ANSI_RESET}")));
        assert!(formatted.contains(" informational diagnostic"));
    }
}
