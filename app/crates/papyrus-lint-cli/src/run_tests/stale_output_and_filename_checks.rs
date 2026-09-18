//! `stale-compiled-output`, `script-filename-mismatch`, and
//! `conflicting-script-versions` diagnostics, plus their disable comments.

use crate::test_support::*;
use std::fs;

#[test]
fn flags_a_script_newer_than_its_compiled_pex() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[info]"), "stdout: {stdout}");
}

#[test]
fn does_not_flag_a_script_older_than_its_compiled_pex() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");

    let now = std::time::SystemTime::now();
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set script mtime");
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file.set_modified(now).expect("failed to set pex mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
}

#[test]
fn stale_compiled_output_can_be_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  stale_compiled_output: false\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
}

#[test]
fn flags_a_script_name_that_does_not_match_its_file_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(
        stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[error]"), "stdout: {stdout}");
}

#[test]
fn does_not_flag_a_script_name_matching_its_file_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(&script_path, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  script_filename_mismatch: false\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_suppressed_with_a_disable_comment() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_suppressed_with_a_disable_file_comment() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

// Regression tests for
// https://github.com/Idrinth/papyrus-lint/issues/772: `stale-compiled-output`,
// `conflicting-script-versions`, and `script-filename-mismatch` are
// computed after `papyrus_lints::lint_with_external_arguments` used to
// run its own unused-directive validation, so an `@disable`/
// `@disable-file` directive that correctly suppressed one of them was
// still reported as an `unused-disable`, even though the diagnostic it
// named was in fact suppressed.

#[test]
fn stale_compiled_output_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(
        &script_path,
        "ScriptName Example ; @disable stale-compiled-output\n",
    );
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn stale_compiled_output_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file stale-compiled-output\n",
    );
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn conflicting_script_versions_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable conflicting-script-versions\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[conflicting-script-versions]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn conflicting_script_versions_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file conflicting-script-versions\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[conflicting-script-versions]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn script_filename_mismatch_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn script_filename_mismatch_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}
