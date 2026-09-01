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
