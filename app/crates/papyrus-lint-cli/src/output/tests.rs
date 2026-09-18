use crate::test_support::*;
use std::fs;

#[test]
fn output_flag_writes_the_plain_text_report_to_a_file_instead_of_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, stderr) = run_captured(&[
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.is_empty());
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(contents.contains("[trailing-whitespace]"));
    assert!(contents.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn output_flag_writes_the_json_report_to_a_file_instead_of_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.json");

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.is_empty());
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    let report: serde_json::Value =
        serde_json::from_str(&contents).expect("output file should contain a JSON document");
    assert_eq!(report["success"], true);
    assert_eq!(report["total_diagnostics"], 1);
}

#[test]
fn output_flag_combines_with_fix() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example\n"
    );
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(contents.contains("no problems found in 1 script"));
    assert!(contents.contains("(1 script(s) fixed.)"));
}

#[test]
fn output_flag_errors_when_the_directory_does_not_exist() {
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
    let output_path = dir.path().join("missing-dir/report.txt");

    let (code, _stdout, stderr) = run_captured(&[
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: failed to write"));
}

#[test]
fn output_flag_without_a_value_prints_usage() {
    let (code, _stdout, stderr) = run_captured(&["--output".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn progress_flag_requires_output_in_plain_text_mode() {
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

    let (code, stdout, stderr) = run_captured(&[
        "--progress".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("--progress requires --output"));
}

#[test]
fn progress_flag_requires_output_in_json_mode() {
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

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--progress".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("--progress requires --output"));
}

#[test]
fn progress_flag_prints_a_progress_bar_to_stdout_when_output_is_set() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/One.psc"),
        "ScriptName One\n",
    );
    write_file(
        &dir.path().join("scripts/source/Two.psc"),
        "ScriptName Two\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/One.psc", "scripts/source/Two.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, stderr) = run_captured(&[
        "--progress".to_string(),
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("\rLinting: 1/2 files"));
    assert!(stdout.contains("\rLinting: 2/2 files"));
    assert!(stdout.ends_with('\n'));
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(contents.contains("no problems found in 2 script"));
}

#[test]
fn output_replaces_an_existing_report_instead_of_appending() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("Example.psc");
    let report = dir.path().join("report.txt");
    write_file(&script, "ScriptName Example\n");
    write_file(&report, "stale report contents that must disappear\n");

    let (code, stdout, stderr) = run_captured(&[
        "--output".to_string(),
        report.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(
        fs::read_to_string(report).expect("failed to read report"),
        "PapyrusLinterCLI: no problems found in 1 script(s).\n"
    );
}

#[test]
fn format_flag_rejects_unknown_values_and_conflicts_with_json() {
    let (unknown_code, unknown_stdout, unknown_stderr) =
        run_captured(&["--format=yaml".to_string(), "Example.psc".to_string()]);
    assert_eq!(unknown_code, 2);
    assert!(unknown_stdout.is_empty());
    assert_eq!(
        unknown_stderr,
        "error: --format must be 'plain', 'json', or 'ai', got 'yaml'\n"
    );

    let (conflict_code, conflict_stdout, conflict_stderr) = run_captured(&[
        "--json".to_string(),
        "--format=json".to_string(),
        "Example.psc".to_string(),
    ]);
    assert_eq!(conflict_code, 2);
    assert!(conflict_stdout.is_empty());
    assert_eq!(
        conflict_stderr,
        "error: --json and --format can't be combined\n"
    );
}
