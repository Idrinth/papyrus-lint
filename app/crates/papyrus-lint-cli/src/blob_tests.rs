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
fn blob_json_reports_parse_failures_as_unsuccessful() {
    let (code, stdout, stderr) = run_captured(&[
        "--format=json".to_string(),
        "--blob=ScriptName Example\nFunction Broken(\n".to_string(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], false);
}

#[test]
fn blob_reports_errors_as_a_failure() {
    let (code, stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n".to_string(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 1);
    assert!(stdout.contains("[forbidden-functions]"));
}

#[test]
fn blob_json_report_uses_the_literal_blob_path() {
    let (code, stdout, stderr) = run_captured(&[
        "--format=json".to_string(),
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
        assert!(stderr.contains("--blob can't be combined with --script-root/--progress/--threads"));
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
