use super::*;

#[test]
fn load_compiler_path_returns_none_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        None
    );
}

#[test]
fn load_compiler_path_reads_explicit_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "compiler_path: C:\\Tools\\PapyrusCompiler.exe\n",
    );

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
}

#[test]
fn load_compiler_path_treats_blank_override_as_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "compiler_path: \"   \"\n");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        None
    );
}

#[test]
fn save_compiler_path_trims_the_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    save_compiler_path(dir.path(), Some("  C:\\Tools\\PapyrusCompiler.exe  "))
        .expect("saving compiler path should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
}

#[test]
fn save_compiler_path_persists_override_without_disturbing_lint_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
}

#[test]
fn save_config_preserves_existing_compiler_path_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
}

#[test]
fn save_compiler_path_none_clears_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    save_compiler_path(dir.path(), None).expect("clearing compiler path should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        None
    );
}
