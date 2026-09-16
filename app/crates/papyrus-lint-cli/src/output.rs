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
