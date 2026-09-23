//! Plain-lint and directory/achlist scanning behavior of `run()`.

use crate::test_support::*;
use std::fs;

#[test]
fn errors_when_achlist_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("missing.achlist");

    let (code, _stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn reports_no_problems_for_a_clean_project() {
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

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn json_reports_parse_failures_as_unsuccessful() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\nFunction Broken(\n");

    let (code, stdout, stderr) = run_captured(&[
        "--format=json".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], false);
}
