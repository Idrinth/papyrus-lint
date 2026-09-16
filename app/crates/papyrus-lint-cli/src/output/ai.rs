use super::json::JsonDiagnostic;
use serde::Serialize;

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

/// Assembles the full `--format ai` report from `ai_files` (one entry per
/// script/blob with at least one diagnostic) and the resolved lint
/// configuration used to produce them. Shared by [`crate::run`] and
/// [`crate::run_blob`] so the AI export's schema, header, and rule-detail
/// derivation can't drift between a normal run and a `--blob` one.
pub(crate) fn build_ai_report(
    lint_config: &papyrus_lints::Config,
    ai_files: Vec<AiFileReport>,
    total_diagnostics: usize,
) -> AiReport {
    let total_rule_counts = rule_counts(ai_files.iter().flat_map(|file| file.diagnostics.iter()));
    let total_severity_counts =
        severity_counts(ai_files.iter().flat_map(|file| file.diagnostics.iter()));
    let mut triggered_rules: Vec<&'static str> = ai_files
        .iter()
        .flat_map(|file| file.diagnostics.iter().map(|diagnostic| diagnostic.rule))
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
        schema: "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json",
        header: AiHeader {
            tool: "Papyrus Lint",
            version: crate::VERSION,
            website: "https://papyrus-lint.idrinth.de",
            target_game: "Skyrim SE/AE",
            generated_at: generated_at(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use papyrus_lint_core::content_hash;

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
}
