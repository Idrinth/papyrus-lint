//! Exercises the desktop executable's real process entry point in CLI mode.
//! Unit tests cover `dispatch` directly, but launching the Cargo-provided
//! binary also covers argument collection, console handling, and process exit.

use std::process::Command;

#[test]
fn desktop_binary_forwards_version_requests_to_the_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg("--version")
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("PapyrusLinterCLI {}\n", papyrus_lint_cli::VERSION)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn desktop_binary_forwards_help_requests_and_the_cli_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg("--help")
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        papyrus_lint_cli::USAGE
    );
}

#[test]
fn desktop_binary_lints_a_single_script_as_json() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Clean.psc");
    std::fs::write(&script, "ScriptName Clean\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["files"][0]["path"], script.display().to_string());
    assert_eq!(report["files"][0]["diagnostics"], serde_json::json!([]));
}

#[test]
fn desktop_binary_propagates_an_invalid_input_error() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("Missing.psc");

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg(&missing)
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed to read"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains(&missing.display().to_string()));
}
