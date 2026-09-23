//! `compile_check`/`compiler_path` behavior of `run()`.

use crate::test_support::*;

#[test]
#[cfg(unix)]
fn compile_check_merges_compiler_reported_errors_into_the_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let compiler_path = write_stub_compiler(
        dir.path(),
        "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "compile_check: true\ncompiler_path: {}\n",
            compiler_path.display()
        ),
    );

    let (code, stdout, _stderr) = run_captured(&[
        "--format=json".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("\"rule\": \"compiler-error\""));
    assert!(stdout.contains("custom compiler error"));
}

#[test]
#[cfg(unix)]
fn compile_check_disabled_by_default_ignores_a_failing_compiler() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let compiler_path = write_stub_compiler(
        dir.path(),
        "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
    );
    // No `compile_check: true`, only a configured `compiler_path` — the
    // compiler must never be invoked at all when the setting is off.
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!("compiler_path: {}\n", compiler_path.display()),
    );

    let (code, stdout, _stderr) = run_captured(&[
        "--format=json".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(!stdout.contains("compiler-error"));
}

#[test]
#[cfg(unix)]
fn compile_check_is_ignored_when_no_compiler_path_can_be_resolved() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compile_check: true\n",
    );

    let (code, stdout, _stderr) = run_captured(&[
        "--format=json".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(!stdout.contains("compiler-error"));
}
