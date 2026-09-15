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

#[test]
fn desktop_binary_preserves_the_cli_failure_code_and_json_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Findings.psc");
    std::fs::write(
        &script,
        "ScriptName Findings\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["success"], false);
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "forbidden-function"
    );
    assert_eq!(report["files"][0]["diagnostics"][0]["line"], 4);
}

#[test]
fn desktop_binary_lints_every_script_listed_in_an_achlist() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("First.psc");
    let second = temp.path().join("Second.psc");
    let achlist = temp.path().join("scripts.achlist");
    std::fs::write(&first, "ScriptName First\n").unwrap();
    std::fs::write(&second, "ScriptName Second\n").unwrap();
    std::fs::write(&achlist, r#"["First.psc", "Second.psc"]"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", achlist.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scripts_checked"], 2);
    let paths: Vec<&str> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&first.to_str().unwrap()));
    assert!(paths.contains(&second.to_str().unwrap()));
}

#[test]
fn desktop_binary_rejects_extra_positional_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["First.psc", "Second.psc"])
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        papyrus_lint_cli::USAGE
    );
}
