use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::json::JsonDiagnostic;

#[derive(Debug, Serialize)]
pub struct AiHeader {
    pub tool: &'static str,
    pub version: String,
    pub website: &'static str,
    pub target_game: &'static str,
    pub generated_at: String,
}

/// How an [`AiFileReport`]'s source is represented. The CLI always has a
/// script's contents in hand by the time it builds one (a read failure
/// aborts the whole run earlier instead), so it only ever constructs
/// `Content` or `Hash`; the desktop app's own "Export for AI" feature also
/// constructs `Error` for a file whose source couldn't be re-read at export
/// time (e.g. it was moved or deleted since linting).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AiSource {
    /// The script's full on-disk contents.
    Content { content: String },
    /// The script's content hash instead of its full text, selected via
    /// `--hash-source`/the "Redact source" checkbox so a report can be
    /// handed to an AI without exposing proprietary script text; a viewer
    /// can still tell files apart, or notice a file changed between
    /// exports, from the hash alone.
    Hash { algorithm: String, hash: String },
    /// The source couldn't be read for this export (desktop app only).
    Error { message: String },
}

#[derive(Debug, Serialize)]
pub struct AiFileReport {
    pub path: String,
    pub severity_counts: AiSeverityCounts,
    pub rule_counts: BTreeMap<String, usize>,
    pub diagnostics: Vec<JsonDiagnostic>,
    /// `None` when nothing was attached for this file at all (the desktop
    /// app's own export options, e.g. no source was requested); always
    /// `Some` from the CLI, which never omits it.
    pub source: Option<AiSource>,
}

#[derive(Debug, Serialize)]
pub struct AiFindings {
    pub files: Vec<AiFileReport>,
    pub total_diagnostics: usize,
    pub severity_counts: AiSeverityCounts,
    pub rule_counts: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Serialize)]
pub struct AiSeverityCounts {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
}

pub fn severity_counts<'a>(
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

pub fn rule_counts<'a>(
    diagnostics: impl IntoIterator<Item = &'a JsonDiagnostic>,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for diagnostic in diagnostics {
        *counts.entry(diagnostic.rule.clone()).or_insert(0) += 1;
    }
    counts
}

#[derive(Debug, Serialize)]
pub struct AiRuleDetails {
    pub rule: &'static str,
    pub description: &'static str,
    pub kinds: &'static [&'static str],
    pub importance: papyrus_lints::tags::Importance,
    pub auto_fixable: bool,
    /// This rule's own documentation link (`RuleTags::doc_url`), so the
    /// assistant reading the export can look up its full explanation on the
    /// project website.
    pub doc_url: String,
}

#[derive(Debug, Serialize)]
pub struct AiReport {
    #[serde(rename = "$schema")]
    pub schema: &'static str,
    pub header: AiHeader,
    pub configuration: serde_json::Value,
    pub findings: AiFindings,
    pub rule_details: Vec<AiRuleDetails>,
}

/// The AI export's own website/schema constants: the project's homepage,
/// where an AI reading an export can look up rule/configuration
/// documentation beyond what `rule_details` itself carries, and the JSON
/// schema the export's top-level shape follows. Shared by every AI export
/// (the CLI's `--format ai`, the desktop app's "Export for AI") so both
/// point at the same schema version.
pub const WEBSITE_URL: &str = "https://papyrus-lint.idrinth.de";
pub const AI_EXPORT_SCHEMA_URL: &str =
    "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json";
pub const TOOL_NAME: &str = "Papyrus Lint";
/// The Papyrus dialect/engine version an AI export's findings were produced
/// for, so an AI reading it doesn't have to guess whether a suggestion
/// (e.g. referencing a native type only added in a later game/edition)
/// actually applies. Neither the CLI nor the desktop app has a per-project
/// game/edition setting of its own (see `rules/native-types.yaml`'s shared
/// Skyrim/Fallout 4 fallback), so this is the fixed target their native
/// rule data is written against.
pub const TARGET_GAME: &str = "Skyrim SE/AE";

/// Assembles a full AI export from `ai_files` (one entry per script/blob
/// with at least one diagnostic) and the resolved lint configuration used
/// to produce them. `tool_version` is the caller's own version string (the
/// CLI's crate version, or the desktop app's own version), so the header
/// correctly names whichever tool actually produced the export. Shared by
/// the CLI's `--format ai`/`--blob --format ai` and the desktop app's
/// `format_issues_for_ai_base` Tauri command so the AI export's schema,
/// header, and rule-detail derivation can't drift between them.
pub fn build_ai_report(
    lint_config: &papyrus_lints::Config,
    tool_version: &str,
    ai_files: Vec<AiFileReport>,
    total_diagnostics: usize,
    generated_at: String,
) -> AiReport {
    let total_rule_counts = rule_counts(ai_files.iter().flat_map(|file| file.diagnostics.iter()));
    let total_severity_counts =
        severity_counts(ai_files.iter().flat_map(|file| file.diagnostics.iter()));
    let mut triggered_rules: Vec<&str> = ai_files
        .iter()
        .flat_map(|file| {
            file.diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rule.as_str())
        })
        .collect();
    triggered_rules.sort_unstable();
    triggered_rules.dedup();
    let rule_details = triggered_rules
        .into_iter()
        .filter_map(papyrus_lints::tags::tags_for)
        .map(|tags| AiRuleDetails {
            rule: tags.rule,
            description: tags.description,
            kinds: tags.kinds,
            importance: tags.importance,
            auto_fixable: tags.auto_fixable(),
            doc_url: tags.doc_url(),
        })
        .collect();

    AiReport {
        schema: AI_EXPORT_SCHEMA_URL,
        header: AiHeader {
            tool: TOOL_NAME,
            version: tool_version.to_string(),
            website: WEBSITE_URL,
            target_game: TARGET_GAME,
            generated_at,
        },
        configuration: ai_configuration(lint_config),
        findings: AiFindings {
            files: ai_files,
            total_diagnostics,
            severity_counts: total_severity_counts,
            rule_counts: total_rule_counts,
        },
        rule_details,
    }
}

/// The AI export's own `configuration` shape: the same resolved
/// [`papyrus_lints::Config`] a lint run used, except its `rules` object
/// (58 individual enable flags, each with its own description in the
/// schema) is replaced with a compact, alphabetically sorted
/// `enabled_rules` list of just the ids that are currently on (see
/// [`papyrus_lints::config::Rules::enabled_ids`]) - no information is
/// lost, since a rule absent from the list is simply disabled, but every
/// export no longer repeats a large, mostly-constant block of booleans.
pub fn ai_configuration(config: &papyrus_lints::Config) -> serde_json::Value {
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
/// also produced by JavaScript's `Date.toISOString()`.
pub fn generated_at() -> String {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format_unix_timestamp(elapsed.as_secs(), elapsed.subsec_millis())
}

pub fn format_unix_timestamp(seconds: u64, milliseconds: u32) -> String {
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

#[cfg(test)]
#[path = "ai_tests.rs"]
mod tests;
