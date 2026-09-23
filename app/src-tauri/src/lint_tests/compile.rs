use super::super::*;
use tempfile::tempdir;

#[test]
fn compile_psc_file_rejects_a_blank_compiler_path_before_spawning() {
    assert!(compile_psc_file(
        "Example.psc".to_string(),
        papyrus_lints::Game::Skyrim,
        "  \t".to_string(),
        Vec::new(),
    )
    .is_err());
}

#[test]
fn compile_psc_file_reports_a_spawn_error_from_the_compiler_module() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "ScriptName Example\n").unwrap();

    let error = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        papyrus_lints::Game::Skyrim,
        dir.path()
            .join("missing-compiler")
            .to_string_lossy()
            .into_owned(),
        Vec::new(),
    )
    .unwrap_err();

    assert!(error.contains("failed to run"));
}

#[test]
fn lint_psc_file_ignores_compile_check_when_disabled() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: "/does/not/matter".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
}

#[test]
fn lint_psc_file_ignores_compile_check_when_no_compiler_path_is_set() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: "   ".to_string(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
}

#[test]
#[cfg(unix)]
fn lint_psc_file_merges_in_compiler_reported_errors_when_enabled() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(
            &compiler_path,
            "#!/bin/sh\necho \"Example.psc(3,4): no viable alternative at character ';'\" >&2\nexit 1\n",
        )
        .unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: compiler_path.to_string_lossy().into_owned(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.rule == compile_diagnostics::RULE
            && diagnostic.line == 3
            && diagnostic.column == 4
    }));
}

#[test]
#[cfg(unix)]
fn lint_psc_file_omits_compiler_diagnostics_when_the_compiler_reports_success() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(&compiler_path, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: compiler_path.to_string_lossy().into_owned(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|d| d.rule != compile_diagnostics::RULE));
}

#[test]
fn lint_psc_file_does_not_fail_when_the_configured_compiler_cannot_be_run() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: dir
                .path()
                .join("missing-compiler")
                .to_string_lossy()
                .into_owned(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
}

#[test]
#[cfg(unix)]
fn compile_command_trims_the_executable_path_and_returns_its_output() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "echo command wrapper\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    symlink("/bin/sh", &compiler_path).unwrap();

    let outcome = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        papyrus_lints::Game::Skyrim,
        format!("  {}  ", compiler_path.display()),
        Vec::new(),
    )
    .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.stdout, "command wrapper\n");
}

#[test]
#[cfg(unix)]
fn compile_command_returns_a_failed_compiler_outcome() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(
        &compiler_path,
        "#!/bin/sh\necho compile failed >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let outcome = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        papyrus_lints::Game::Skyrim,
        compiler_path.to_string_lossy().into_owned(),
        Vec::new(),
    )
    .unwrap();

    assert!(!outcome.success);
    assert_eq!(outcome.stderr, "compile failed\n");
}

#[test]
#[cfg(unix)]
fn compile_command_forwards_additional_script_roots() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    let additional_root = dir.path().join("Shared Scripts");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&additional_root).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(&compiler_path, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let outcome = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        papyrus_lints::Game::Skyrim,
        compiler_path.to_string_lossy().into_owned(),
        vec![additional_root.to_string_lossy().into_owned()],
    )
    .unwrap();

    assert!(outcome.success);
    assert!(outcome
        .stdout
        .contains(&additional_root.to_string_lossy().into_owned()));
    assert!(outcome.stdout.contains("Example.psc"));
}
