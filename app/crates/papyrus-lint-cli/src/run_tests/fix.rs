//! `fix` command behavior of `run()`.

use crate::test_support::*;
use std::fs;

#[test]
fn fix_rewrites_fixable_issues_and_reports_the_rest() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
    );
    assert_eq!(code, 1);
    assert!(!stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("Game.GetPlayer"));
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_preserves_a_cp1252_encoded_files_encoding() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
    // "ScriptName Example   \n\n; caf\xE9\n" with trailing whitespace on
    // the first line to fix, and 0xE9 ("é" in Windows-1252) making the
    // file as a whole invalid UTF-8.
    let mut contents = b"ScriptName Example   \n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    fs::write(&script_path, &contents).expect("failed to write test file");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert!(stdout.contains("(1 script(s) fixed.)"), "stderr: {stderr}");
    let mut expected = b"ScriptName Example\n\n; caf".to_vec();
    expected.push(0xE9);
    expected.push(b'\n');
    assert_eq!(
        fs::read(&script_path).expect("failed to read back fixed file"),
        expected,
        "fixing must preserve the file's original Windows-1252 encoding"
    );
    assert_eq!(code, 0);
}

#[test]
fn fix_type_filter_applies_only_the_named_rule() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction Add(Int left,Int right)\nEndFunction   \n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=comma_spacing".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nFunction Add(Int left, Int right)\nEndFunction   \n"
    );
}

#[test]
fn fix_type_filter_accepts_the_hyphenated_rule_id() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type".to_string(),
        "trailing-whitespace".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
}

#[test]
fn fix_line_filter_only_touches_the_named_line() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction DoThing()   \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n\nFunction DoThing()\nEndFunction\n"
    );
}

#[test]
fn fix_line_and_type_filters_combine() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction Add(Int left,Int right)   \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        "--type=comma_spacing".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n\nFunction Add(Int left, Int right)   \nEndFunction\n"
    );
}

#[test]
fn line_filter_errors_when_a_fix_changes_the_line_count() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  property_sorting: true\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("changes the file's line count"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n"
    );
}

#[test]
fn fix_does_not_rewrite_an_already_clean_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    assert!(stdout.contains("(0 script(s) fixed.)"));
}

#[test]
fn fix_dry_run_prints_a_diff_without_writing_the_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    let original = "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n";
    write_file(&script_path, original);

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        original,
        "--dry-run must never write to the file"
    );
    let expected_path = script_path.to_string_lossy();
    assert!(stdout.contains(&format!("--- {expected_path}\n")));
    assert!(stdout.contains(&format!("+++ {expected_path}\n")));
    assert!(stdout.contains("-ScriptName Example   \n"));
    assert!(stdout.contains("+ScriptName Example\n"));
    assert!(stdout.contains("(1 script(s) would be fixed.)"));
    assert!(stdout.contains("Game.GetPlayer"));
}

#[test]
fn fix_dry_run_prints_nothing_extra_for_an_already_clean_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    assert!(!stdout.contains("---"));
    assert!(stdout.contains("(0 script(s) would be fixed.)"));
}

#[test]
fn fix_errors_when_the_achlist_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("missing.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn fix_removes_an_unused_import_resolved_through_the_project_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nFunction Assist() Global\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stdout: {stdout}, stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_type_filter_for_unused_import_only_touches_that_rule() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nFunction Assist() Global\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=unused-import".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
    );
}

#[test]
fn fix_line_filter_errors_when_removing_an_unused_import_changes_the_line_count() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nFunction Assist() Global\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("changes the file's line count"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n"
    );
}
