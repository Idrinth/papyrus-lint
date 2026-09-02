//! End-to-end tests for the standalone `PapyrusLinterCLI` binary.
//!
//! The unit tests in `src/lib.rs` exercise the shared `run` function. These
//! tests additionally verify that the binary entry point forwards arguments,
//! writes to the expected process streams, and returns the documented status.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directory");
    }
    fs::write(path, contents).expect("failed to write fixture");
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
        .args(args)
        .output()
        .expect("failed to run PapyrusLinterCLI")
}

fn run_cli_in(args: &[&str], current_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("failed to run PapyrusLinterCLI")
}

#[test]
fn help_is_written_to_stderr_with_the_usage_error_status() {
    let output = run_cli(&["--help"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn version_is_written_to_stdout() {
    let output = run_cli(&["--version"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!("PapyrusLinterCLI {}\n", env!("CARGO_PKG_VERSION"))
    );
}

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
}

#[test]
fn fix_mode_rewrites_a_script_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["fix", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example\n"
    );
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be UTF-8")
        .contains("(1 script(s) fixed.)"));
}

#[test]
fn init_creates_a_config_in_the_process_working_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(&["init"], dir.path());

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!(
            "Created {}\n",
            dir.path().join("papyrus-lint.yaml").display()
        )
    );
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("trailing_whitespace: true"));
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
fn missing_script_reports_an_io_error_on_stderr() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let missing_script = dir.path().join("scripts/source/Missing.psc");

    let output = run_cli(&[&missing_script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to read "));
    assert!(stderr.contains("Missing.psc"));
}

#[test]
fn lint_errors_produce_a_failure_status_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
    );

    let output = run_cli(&[&script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[forbidden-functions]"));
    assert!(stdout.contains("[error]"));
    assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn achlist_json_report_includes_clean_and_dirty_scripts() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let clean_script = dir.path().join("scripts/source/Clean.psc");
    let dirty_script = dir.path().join("scripts/source/Dirty.psc");
    let achlist = dir.path().join("sources.achlist");
    write_file(&clean_script, "ScriptName Clean\n");
    write_file(&dirty_script, "ScriptName Dirty   \n");
    write_file(
        &achlist,
        r#"["scripts/source/Clean.psc", "scripts/source/Dirty.psc"]"#,
    );

    let output = run_cli(&["--json", "--short-paths", &achlist.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 2);
    assert_eq!(report["files_with_diagnostics"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    let files = report["files"]
        .as_array()
        .expect("files should be an array");
    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0]["path"],
        Path::new("scripts/source/Clean.psc")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(
        files[1]["path"],
        Path::new("scripts/source/Dirty.psc")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(files[1]["diagnostics"][0]["rule"], "trailing-whitespace");
}

#[test]
fn invalid_config_is_reported_by_the_binary_without_a_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules: not-a-rule-map\n",
    );

    let output = run_cli(&["--json", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to load lint config:"));
    assert!(stderr.contains("expected struct Rules"));
}

#[test]
fn init_refuses_to_replace_an_existing_config_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let config_path = dir.path().join("papyrus-lint.yaml");
    write_file(&config_path, "rules:\n  semicolon: false\n");

    let output = run_cli_in(&["init"], dir.path());

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .contains("config already exists"));
    assert_eq!(
        fs::read_to_string(config_path).expect("existing config should remain readable"),
        "rules:\n  semicolon: false\n"
    );
}
