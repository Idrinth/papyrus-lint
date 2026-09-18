mod ai;
mod json;
mod plain;

pub(crate) use ai::*;
pub(crate) use json::*;
pub use json::{JsonDiagnostic, JsonFileReport, JsonReport};
pub(crate) use plain::*;

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
    use crate::test_support::*;
    use std::fs;

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
        assert!(contents.contains("(trailing-whitespace)"));
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
}
