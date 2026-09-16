//! End-to-end tests for report formatting (`--json`, `--format`, `--color`,
//! `--output`, `--progress`) through the standalone `PapyrusLinterCLI` binary.
//! Unit tests for the formatters live in `src/output.rs`.

mod common;

use common::*;
use std::fs;
use std::path::Path;

#[test]
fn json_mode_lints_a_script_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--json", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
    assert_eq!(
        report["files"][0]["diagnostics"][0]["doc_url"],
        "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace"
    );
}

#[test]
fn plain_text_output_links_a_tagged_rule_to_its_documentation() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&[&script.to_string_lossy()]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace"));
}

#[test]
fn ai_format_includes_source_and_triggered_rule_details() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let source = "ScriptName Example   \n";
    write_file(&script, source);

    let output = run_cli(&["--format", "ai", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain an AI export");
    assert_eq!(
        report["$schema"],
        "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json"
    );
    assert_eq!(report["header"]["tool"], "Papyrus Lint");
    assert_eq!(
        report["header"]["website"],
        "https://papyrus-lint.idrinth.de"
    );
    assert_eq!(report["header"]["target_game"], "Skyrim SE/AE");
    let generated_at = report["header"]["generated_at"]
        .as_str()
        .expect("generated_at should be a string");
    assert_eq!(generated_at.len(), 24);
    assert!(generated_at.ends_with('Z'));
    assert_eq!(report["configuration"]["semicolon"], false);
    assert!(report["configuration"]["rules"].is_null());
    let enabled_rules = report["configuration"]["enabled_rules"]
        .as_array()
        .expect("enabled_rules should be an array");
    assert!(enabled_rules.contains(&serde_json::json!("trailing-whitespace")));
    assert!(!enabled_rules.contains(&serde_json::json!("property-sorting")));
    assert_eq!(report["findings"]["files"][0]["source"]["type"], "content");
    assert_eq!(report["findings"]["files"][0]["source"]["content"], source);
    assert_eq!(
        report["findings"]["files"][0]["severity_counts"],
        serde_json::json!({"errors": 0, "warnings": 1, "info": 0})
    );
    assert_eq!(
        report["findings"]["files"][0]["rule_counts"]["trailing-whitespace"],
        1
    );
    assert_eq!(report["findings"]["rule_counts"]["trailing-whitespace"], 1);
    assert_eq!(
        report["findings"]["severity_counts"],
        serde_json::json!({"errors": 0, "warnings": 1, "info": 0})
    );
    assert_eq!(
        report["findings"]["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
    assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
    assert!(report["rule_details"][0]["description"].is_string());
    assert_eq!(report["rule_details"][0]["auto_fixable"], true);
    assert_eq!(
        report["rule_details"][0]["doc_url"],
        "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace"
    );
}

#[test]
fn ai_format_omits_scripts_without_diagnostics() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    write_file(
        &scripts.join("WithIssues.psc"),
        "ScriptName WithIssues   \n",
    );
    write_file(&scripts.join("Clean.psc"), "ScriptName Clean\n");

    let output = run_cli(&["--format", "ai", &scripts.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain an AI export");
    let files = report["findings"]["files"]
        .as_array()
        .expect("AI export files should be an array");
    assert_eq!(files.len(), 1);
    assert!(files[0]["path"]
        .as_str()
        .expect("AI export path should be a string")
        .ends_with("WithIssues.psc"));
    assert!(!output
        .stdout
        .windows(b"Clean.psc".len())
        .any(|window| window == b"Clean.psc"));
}

#[test]
fn ai_format_hash_source_reports_an_md5_digest_instead_of_the_full_content() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let source = "ScriptName Example   \n";
    write_file(&script, source);

    let output = run_cli(&["--format", "ai", "--hash-source", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain an AI export");
    assert_eq!(report["findings"]["files"][0]["source"]["type"], "hash");
    assert_eq!(report["findings"]["files"][0]["source"]["algorithm"], "md5");
    assert_eq!(
        report["findings"]["files"][0]["source"]["hash"],
        papyrus_lint_core::content_hash::md5_hex(source)
    );
    assert!(!output
        .stdout
        .windows(source.len())
        .any(|window| window == source.as_bytes()));
}

#[test]
fn hash_source_without_format_ai_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&["--hash-source", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --hash-source requires --format ai\n"
    );
}

#[test]
fn output_flag_redirects_json_without_writing_to_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let report_path = dir.path().join("reports/lint.json");
    write_file(&script, "ScriptName Example   \n");
    fs::create_dir_all(report_path.parent().unwrap()).expect("failed to create reports directory");

    let output = run_cli(&[
        "--json",
        "--output",
        &report_path.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(report_path).expect("JSON report should be written"),
    )
    .expect("output file should contain JSON");
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
}

#[test]
fn color_always_emits_ansi_escapes_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--color=always", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains('\x1b'));
}

#[test]
fn invalid_color_value_reports_usage_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&["--color", "sometimes", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert_eq!(
        stderr,
        "error: --color must be 'auto', 'always', or 'never', got 'sometimes'\n"
    );
}

#[test]
fn progress_keeps_the_report_in_its_output_file_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    let report_path = dir.path().join("lint-report.json");
    write_file(&scripts.join("One.psc"), "ScriptName One\n");
    write_file(&scripts.join("Two.psc"), "ScriptName Two   \n");

    let output = run_cli(&[
        "--json",
        "--progress",
        "--output",
        &report_path.to_string_lossy(),
        &scripts.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("\rLinting: 1/2 files"));
    assert!(stdout.contains("\rLinting: 2/2 files"));
    assert!(!stdout.contains("\"scripts_checked\""));

    let report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(report_path).expect("JSON report should be written"),
    )
    .expect("output file should contain JSON");
    assert_eq!(report["scripts_checked"], 2);
    assert_eq!(report["total_diagnostics"], 1);
}

#[test]
fn format_json_equals_form_lints_a_relative_script_path() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let relative_script = Path::new("scripts/source/Example.psc");
    write_file(&dir.path().join(relative_script), "ScriptName Example   \n");

    let output = run_cli_in(&["--format=json", "scripts/source/Example.psc"], dir.path());

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
}

#[test]
fn json_dry_run_reports_the_diff_without_rewriting_the_script() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let original = "ScriptName Example   \n";
    write_file(&script, original);

    let output = run_cli(&["--json", "fix", "--dry-run", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(&script).expect("script should remain readable"),
        original
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["files_fixed"], 1);
    let diff = report["files"][0]["diff"]
        .as_str()
        .expect("changed file should include a diff");
    assert!(diff.contains("-ScriptName Example   \n"));
    assert!(diff.contains("+ScriptName Example\n"));
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .is_empty());
}

#[test]
fn plain_output_file_receives_the_report_without_stdout_noise() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let report_path = dir.path().join("reports/lint.txt");
    write_file(&script, "ScriptName Example   \n");
    fs::create_dir_all(report_path.parent().expect("report should have a parent"))
        .expect("failed to create report directory");

    let output = run_cli(&[
        "--output",
        &report_path.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let report = fs::read_to_string(report_path).expect("plain report should be readable");
    assert!(report.contains("trailing-whitespace"));
    assert!(report.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn unknown_format_is_a_usage_error_without_creating_an_output_file() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let report_path = dir.path().join("lint.txt");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&[
        "--format",
        "xml",
        "--output",
        &report_path.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --format must be 'plain', 'json', or 'ai', got 'xml'\n"
    );
    assert!(!report_path.exists());
}

#[test]
fn json_flag_cannot_be_combined_with_an_explicit_format() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&["--json", "--format=plain", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --json and --format can't be combined\n"
    );
}

#[test]
fn progress_without_an_output_file_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&["--progress", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --progress requires --output <path>\n"
    );
}
