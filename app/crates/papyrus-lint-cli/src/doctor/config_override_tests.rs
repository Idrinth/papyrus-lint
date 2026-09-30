use crate::test_support::*;

#[test]
fn doctor_honors_compile_check_from_an_explicit_config_path() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "game: skyrim\ncompile_check: false\n",
    );
    let override_config = dir.path().join("over.yaml");
    write_file(&override_config, "game: skyrim\ncompile_check: true\n");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        override_config.to_string_lossy().into_owned(),
        dir.path().to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("explicit config"));
    assert!(stdout.contains(
        "[warning] compile_check is enabled but no PapyrusCompiler.exe could be resolved"
    ));
}

#[test]
fn doctor_honors_compiler_path_from_an_explicit_config_path() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compiler_path: /from-project/PapyrusCompiler.exe\n",
    );
    let override_config = dir.path().join("over.yaml");
    write_file(
        &override_config,
        "compiler_path: /from-override/PapyrusCompiler.exe\n",
    );

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        override_config.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("configured compiler_path /from-override/PapyrusCompiler.exe"));
    assert!(!stdout.contains("/from-project/PapyrusCompiler.exe"));
}
