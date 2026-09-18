//! End-to-end tests for `--blob` through the standalone `PapyrusLinterCLI`
//! binary. Unit tests for `run_blob` live in `src/blob.rs`.

mod common;

use common::*;

#[test]
fn blob_mode_lints_unsaved_source_through_the_binary_entry_point() {
    let source = "ScriptName Unsaved   \n";

    let output = run_cli(&["--blob", source]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("<blob>:1:19:") && stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("1 problem(s) found in the given blob"));
}

#[test]
fn blob_json_honors_an_explicit_config_and_failure_threshold() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let config = dir.path().join("strict-warnings.yaml");
    write_file(&config, "fail_on_warning: true\n");

    let output = run_cli(&[
        "--blob=ScriptName Unsaved   \n",
        "--format=json",
        "--config",
        &config.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["files"][0]["path"], "<blob>");
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
    assert_eq!(report["total_diagnostics"], 1);
    assert_eq!(report["success"], false);
}

#[test]
fn blob_ai_hash_export_does_not_expose_the_source() {
    let source = "ScriptName Secret   \n";

    let output = run_cli(&["--format=ai", "--hash-source", "--blob", source]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain an AI export");
    let exported_source = &report["findings"]["files"][0]["source"];
    assert_eq!(exported_source["type"], "hash");
    assert_eq!(exported_source["algorithm"], "md5");
    assert_eq!(
        exported_source["hash"],
        papyrus_lint_core::content_hash::md5_hex(source)
    );
    assert!(!String::from_utf8(output.stdout)
        .expect("stdout should be UTF-8")
        .contains(source));
}

#[test]
fn blob_mode_rejects_file_only_options_before_writing_an_output() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let report_path = dir.path().join("report.json");

    let output = run_cli(&[
        "--blob",
        "ScriptName Unsaved\n",
        "--threads=2",
        "--output",
        &report_path.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --blob can't be combined with --script-root/--progress/--threads\n"
    );
    assert!(!report_path.exists());
}
