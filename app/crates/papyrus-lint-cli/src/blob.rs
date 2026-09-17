use std::io::Write;
use std::path::Path;

use papyrus_lint_config as config;
use papyrus_lint_core::content_hash;

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
/// `fail_on_warning`/`fail_on_info`), `1` if any did, or `2` on a `--config`
/// load failure or a failure to write `--output <path>`.
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
    let lint_config = match config_path {
        Some(path) => match config::load_config_from_path(path) {
            Ok(config) => config,
            Err(err) => {
                let _ = writeln!(stderr, "error: failed to load lint config: {err}");
                return 2;
            }
        },
        None => papyrus_lints::Config::default(),
    };

    let mut diagnostics = papyrus_lints::lint(source, &lint_config);
    let should_fail = finalize_diagnostics(
        &mut diagnostics,
        &lint_config,
        tag_filter,
        quiet_warnings,
        quiet_info,
    );

    let use_color = resolve_color(color_choice, output_path, stdout_is_terminal);

    let total_diagnostics = diagnostics.len();
    let json_diagnostics = to_json_diagnostics(&diagnostics);

    let mut report_buf: Vec<u8> = Vec::new();

    match output_format {
        OutputFormat::Json => write_blob_json(
            &mut report_buf,
            json_diagnostics,
            total_diagnostics,
            should_fail,
        ),
        OutputFormat::Ai => write_blob_ai(
            &mut report_buf,
            &lint_config,
            json_diagnostics,
            source,
            hash_source,
            total_diagnostics,
        ),
        OutputFormat::Plain => write_blob_plain(
            &mut report_buf,
            &diagnostics,
            total_diagnostics,
            should_fail,
            use_color,
        ),
    }

    let write_status = flush_report(&report_buf, output_path, stdout, stderr);
    if write_status != 0 {
        return write_status;
    }

    if should_fail {
        1
    } else {
        0
    }
}

fn write_blob_json(
    report_buf: &mut Vec<u8>,
    json_diagnostics: Vec<JsonDiagnostic>,
    total_diagnostics: usize,
    should_fail: bool,
) {
    let report = JsonReport {
        files: vec![JsonFileReport {
            path: BLOB_PATH.to_string(),
            diagnostics: json_diagnostics,
            diff: None,
        }],
        scripts_checked: 1,
        files_with_diagnostics: if total_diagnostics > 0 { 1 } else { 0 },
        total_diagnostics,
        files_fixed: None,
        dry_run: false,
        success: !should_fail,
    };
    write_json_report(report_buf, &report);
}

fn write_blob_ai(
    report_buf: &mut Vec<u8>,
    lint_config: &papyrus_lints::Config,
    json_diagnostics: Vec<JsonDiagnostic>,
    source: &str,
    hash_source: bool,
    total_diagnostics: usize,
) {
    let ai_files = if json_diagnostics.is_empty() {
        Vec::new()
    } else {
        let rule_counts = rule_counts(&json_diagnostics);
        let severity_counts = severity_counts(&json_diagnostics);
        let ai_source = if hash_source {
            AiSource::Hash {
                algorithm: "md5",
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
            source: ai_source,
        }]
    };
    let report = build_ai_report(lint_config, ai_files, total_diagnostics);
    write_json_report(report_buf, &report);
}

fn write_blob_plain(
    report_buf: &mut Vec<u8>,
    diagnostics: &[papyrus_lints::Diagnostic],
    total_diagnostics: usize,
    should_fail: bool,
    use_color: bool,
) {
    for diagnostic in diagnostics {
        let _ = writeln!(
            report_buf,
            "{}",
            format_diagnostic_line(BLOB_PATH, diagnostic, use_color)
        );
    }
    let summary_color = if total_diagnostics == 0 {
        ANSI_GREEN
    } else if should_fail {
        ANSI_RED
    } else {
        ANSI_YELLOW
    };
    let summary = if total_diagnostics == 0 {
        "PapyrusLinterCLI: no problems found in the given blob.".to_string()
    } else {
        format!("PapyrusLinterCLI: {total_diagnostics} problem(s) found in the given blob.")
    };
    let _ = writeln!(
        report_buf,
        "{}",
        colorize(&summary, summary_color, use_color)
    );
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use papyrus_lint_core::content_hash;
    use std::fs;

    #[test]
    fn blob_lints_raw_source_text_without_a_file() {
        let (code, stdout, stderr) =
            run_captured(&["--blob".to_string(), "ScriptName Example   \n".to_string()]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(stdout.contains("<blob>:1:"));
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("problem(s) found in the given blob"));
    }

    #[test]
    fn blob_with_no_diagnostics_reports_success() {
        let (code, stdout, stderr) = run_captured(&["--blob=ScriptName Example\n".to_string()]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in the given blob"));
    }

    #[test]
    fn blob_reports_errors_as_a_failure() {
        let (code, stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
                .to_string(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 1);
        assert!(stdout.contains("[forbidden-functions]"));
    }

    #[test]
    fn blob_json_report_uses_the_literal_blob_path() {
        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "--blob".to_string(),
            "ScriptName Example   \n".to_string(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["scripts_checked"], 1);
        assert_eq!(report["files"][0]["path"], "<blob>");
        assert_eq!(
            report["files"][0]["diagnostics"][0]["rule"],
            "trailing-whitespace"
        );
    }

    #[test]
    fn blob_honors_tag_filter() {
        let (code, stdout, stderr) = run_captured(&[
            "--tag=performance".to_string(),
            "--blob".to_string(),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
                .to_string(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 1);
        assert!(stdout.contains("[forbidden-functions]"));
        assert!(!stdout.contains("[trailing-whitespace]"));
    }

    #[test]
    fn blob_honors_an_explicit_config_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_path = dir.path().join("papyrus-lint.yaml");
        write_file(&config_path, "rules:\n  trailing_whitespace: false\n");

        let (code, stdout, stderr) = run_captured(&[
            "--config".to_string(),
            config_path.to_string_lossy().into_owned(),
            "--blob".to_string(),
            "ScriptName Example   \n".to_string(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(!stdout.contains("[trailing-whitespace]"));
    }

    #[test]
    fn blob_reports_a_missing_explicit_config_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let missing_config = dir.path().join("missing.yaml");

        let (code, stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
            "--config".to_string(),
            missing_config.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.contains("error: failed to load lint config:"));
        assert!(stderr.contains("No such file or directory"));
    }

    #[test]
    fn blob_ai_format_reports_content_and_rule_metadata() {
        let source = "ScriptName Example   \n";

        let (code, stdout, stderr) = run_captured(&[
            "--format=ai".to_string(),
            "--blob".to_string(),
            source.to_string(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stderr.is_empty());
        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("AI report should be valid JSON");
        assert_eq!(report["findings"]["total_diagnostics"], 1);
        assert_eq!(report["findings"]["files"][0]["path"], "<blob>");
        assert_eq!(report["findings"]["files"][0]["source"]["content"], source);
        assert_eq!(report["findings"]["rule_counts"]["trailing-whitespace"], 1);
        assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
        assert!(report["configuration"]["enabled_rules"].is_array());
    }

    #[test]
    fn blob_ai_hash_source_omits_the_source_content() {
        let source = "ScriptName Example   \n";

        let (code, stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            source.to_string(),
            "--format".to_string(),
            "ai".to_string(),
            "--hash-source".to_string(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stderr.is_empty());
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
    fn blob_ai_format_omits_clean_files_from_findings() {
        let (code, stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
            "--format=ai".to_string(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stderr.is_empty());
        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("AI report should be valid JSON");
        assert_eq!(report["findings"]["total_diagnostics"], 0);
        assert_eq!(report["findings"]["files"], serde_json::json!([]));
        assert_eq!(report["rule_details"], serde_json::json!([]));
    }

    #[test]
    fn blob_reports_an_error_when_output_cannot_be_written() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let (code, stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
            "--output".to_string(),
            dir.path().to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.contains("error: failed to write"));
        assert!(stderr.contains(&dir.path().display().to_string()));
    }

    #[test]
    fn blob_rejects_an_unknown_tag() {
        let (code, _stdout, stderr) = run_captured(&[
            "--tag=made-up-tag".to_string(),
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("unknown tag 'made-up-tag'"));
    }

    #[test]
    fn blob_cannot_be_combined_with_a_path_argument() {
        let (code, _stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
            "some/path.psc".to_string(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--blob can't be combined"));
    }

    #[test]
    fn blob_cannot_be_combined_with_fix() {
        let (code, _stdout, stderr) = run_captured(&[
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
            "fix".to_string(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--blob can't be combined"));
    }

    #[test]
    fn blob_cannot_be_combined_with_dry_run_type_or_line() {
        for flag in ["--dry-run", "--type=trailing-whitespace", "--line=1"] {
            let (code, _stdout, stderr) = run_captured(&[
                flag.to_string(),
                "--blob".to_string(),
                "ScriptName Example\n".to_string(),
            ]);

            assert_eq!(code, 2, "flag {flag} should have been rejected");
            assert!(stderr.contains("--blob can't be combined with fix/--type/--line/--dry-run"));
        }
    }

    #[test]
    fn blob_cannot_be_combined_with_script_root_progress_or_threads() {
        for args in [
            vec!["--script-root".to_string(), "other".to_string()],
            vec![
                "--progress".to_string(),
                "--output".to_string(),
                "out.txt".to_string(),
            ],
            vec!["--threads=2".to_string()],
        ] {
            let mut full_args = args;
            full_args.push("--blob".to_string());
            full_args.push("ScriptName Example\n".to_string());

            let (code, _stdout, stderr) = run_captured(&full_args);

            assert_eq!(code, 2, "args {full_args:?} should have been rejected");
            assert!(
                stderr.contains("--blob can't be combined with --script-root/--progress/--threads")
            );
        }
    }

    #[test]
    fn blob_output_flag_redirects_the_report_to_a_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let output_path = dir.path().join("report.txt");

        let (code, stdout, stderr) = run_captured(&[
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            "--blob".to_string(),
            "ScriptName Example   \n".to_string(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(stdout.is_empty());
        let report = fs::read_to_string(&output_path).unwrap();
        assert!(report.contains("[trailing-whitespace]"));
    }
}
