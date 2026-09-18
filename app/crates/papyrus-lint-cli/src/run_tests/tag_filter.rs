//! `--tag` filtering of reported and fixed diagnostics.

use crate::test_support::*;
use std::fs;

#[test]
fn tag_filter_restricts_reported_diagnostics_to_the_matching_kind() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );

    let (code, stdout, stderr) = run_captured(&[
        "--tag=style".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains("[forbidden-functions]"));
}

#[test]
fn tag_filter_matches_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[
        "--tag=STYLE".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
}

#[test]
fn tag_filter_works_without_fix() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "--tag=performance".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn fix_tag_filter_applies_only_fixes_in_that_kind() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()  \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--tag=performance".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    let fixed = fs::read_to_string(&script_path).unwrap();
    assert!(!fixed.contains("GetValueInt"));
    // trailing-whitespace is tagged "style", not "performance", so it's
    // left in place by --tag=performance.
    assert!(fixed.contains("  \n"));
}
