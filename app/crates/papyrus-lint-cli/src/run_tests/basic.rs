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
fn reports_diagnostics_and_exits_1_for_a_dirty_project() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[forbidden-functions]"));
    assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn does_not_fail_on_warning_level_diagnostics_by_default() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("[unused-property]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn fails_on_warning_level_diagnostics_when_configured() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_warning: true\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[unused-property]"));
}

#[test]
fn quiet_warnings_hides_warnings_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_warning: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[
        "--quiet-warnings".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(!stdout.contains("[warning]"));
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn quiet_info_hides_info_diagnostics_from_json_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nGlobalVariable Property Value Auto\n\nFunction Test()\n    Value.GetValueInt()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_info: true\n",
    );

    let (unfiltered_code, unfiltered_stdout, _) = run_captured(&[
        "--format=json".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);
    let unfiltered: serde_json::Value = serde_json::from_str(&unfiltered_stdout).unwrap();
    assert_eq!(unfiltered_code, 1);
    assert!(unfiltered["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["level"] == "info"));

    let (code, stdout, stderr) = run_captured(&[
        "--format=json".to_string(),
        "--quiet-info".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], false);
    let diagnostics = report["files"][0]["diagnostics"].as_array().unwrap();
    assert_eq!(report["total_diagnostics"], diagnostics.len());
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic["level"] != "info"));
}

#[test]
fn honors_the_project_yaml_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn reports_an_invalid_project_yaml_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules: definitely-not-a-rule-set\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("error: failed to load lint config:"));
}

#[test]
fn ignores_non_psc_entries_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(&dir.path().join("scripts/source/Example.pex"), "");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.pex"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 0 script"));
}

#[test]
fn recognizes_uppercase_psc_extensions_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.PSC"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.PSC"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn lints_a_single_psc_file_passed_directly() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn reports_no_problems_for_a_clean_single_psc_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn lints_every_psc_found_recursively_under_a_dropped_directory() {
    // Mirrors a mod like Requiem, whose scripts are spread across
    // arbitrarily nested subfolders rather than a flat scripts/source
    // directory, and ships no .achlist at all.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Top.psc"),
        "ScriptName Top   \n",
    );
    write_file(
        &dir.path().join("scripts/source/Requiem/Sub/Nested.psc"),
        "ScriptName Nested   \n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("2 script(s)"));
    assert!(stdout.contains("Top.psc"));
    assert!(stdout.contains("Nested.psc"));
}

#[test]
fn directory_scan_reports_no_problems_for_an_empty_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 0 script"));
}

#[test]
fn directory_scan_resolves_cross_script_calls_across_nested_subfolders() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Requiem/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn same_script_lints_identically_via_achlist_and_directly_when_namespaced() {
    // The same nested script should produce the same diagnostics
    // whether it's resolved from an .achlist or linted directly.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/User/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Greeter.psc", "scripts/source/User/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (achlist_code, achlist_stdout, _) =
        run_captured(&[achlist_path.to_string_lossy().into_owned()]);
    let (direct_code, direct_stdout, _) =
        run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(achlist_code, 1);
    assert_eq!(direct_code, 1);
    assert!(achlist_stdout.contains("[argument-types]"));
    assert!(direct_stdout.contains("[argument-types]"));
}

#[test]
fn decodes_a_cp1252_encoded_script_instead_of_failing_the_whole_run() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
    // "; caf\xE9" in Windows-1252 (0xE9 is "é"), which is not valid
    // UTF-8 on its own.
    let mut contents = b"ScriptName Example\n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    fs::write(&script_path, &contents).expect("failed to write test file");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn errors_when_the_given_psc_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Missing.psc");

    let (code, _stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn a_psc_path_that_is_a_directory_reports_a_read_error() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let misleading_path = dir.path().join("NotAFile.psc");
    fs::create_dir(&misleading_path).expect("failed to create directory");

    let (code, stdout, stderr) = run_captured(&[misleading_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("error: failed to read"));
    assert!(stderr.contains("NotAFile.psc"));
}

#[test]
fn direct_psc_detection_is_case_insensitive() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("Example.PSC");
    write_file(&script, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}
