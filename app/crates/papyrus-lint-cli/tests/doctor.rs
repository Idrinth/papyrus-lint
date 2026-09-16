//! End-to-end tests for `doctor` through the standalone `PapyrusLinterCLI`
//! binary. Unit tests for `run_doctor` live in `src/doctor.rs`.

mod common;

use common::*;
use std::fs;

#[test]
fn doctor_reports_a_healthy_project_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&["doctor", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains(&format!("[ok] script {} exists", script.display())));
    assert!(stdout.contains("[ok] no papyrus-lint.yaml/.yml found; using default settings"));
    assert!(stdout.contains("PapyrusLinterCLI doctor: no problems found."));
}

#[test]
fn doctor_json_reports_failed_checks_and_a_failure_status_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let missing_script = dir.path().join("scripts/source/Missing.psc");

    let output = run_cli(&["doctor", "--json", &missing_script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["success"], false);
    assert_eq!(
        report["project_root"],
        dir.path().to_string_lossy().as_ref()
    );
    let checks = report["checks"]
        .as_array()
        .expect("doctor checks should be an array");
    assert!(checks.iter().any(|check| {
        check["status"] == "error"
            && check["message"]
                .as_str()
                .is_some_and(|message| message.contains("Missing.psc does not exist"))
    }));
    assert!(checks.iter().any(|check| check["status"] == "warning"));
}

#[test]
fn doctor_checks_each_cli_script_root_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let shared_scripts = dir.path().join("shared-scripts");
    write_file(&script, "ScriptName Example\n");
    fs::create_dir_all(&shared_scripts).expect("failed to create additional script root");

    let output = run_cli(&[
        "doctor",
        "--script-root",
        "shared-scripts",
        "--script-root",
        "missing-scripts",
        &script.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains(&format!(
        "[ok] additional script root {} exists",
        shared_scripts.display()
    )));
    assert!(stdout.contains(&format!(
        "[warning] configured additional script root {} does not exist",
        dir.path().join("missing-scripts").display()
    )));
    assert!(stdout.contains("PapyrusLinterCLI doctor: 1 problem(s) found."));
}
