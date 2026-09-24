use super::*;

#[test]
fn load_compile_check_defaults_to_false_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert!(!load_compile_check(dir.path()).expect("should succeed"));
}

#[test]
fn save_and_load_compile_check_round_trips_without_disturbing_lint_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");
    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    save_compile_check(dir.path(), true).expect("saving compile check should succeed");

    assert!(load_compile_check(dir.path()).expect("should succeed"));
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
}

#[test]
fn save_compile_check_false_clears_it() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_compile_check(dir.path(), true).expect("saving compile check should succeed");

    save_compile_check(dir.path(), false).expect("clearing compile check should succeed");

    assert!(!load_compile_check(dir.path()).expect("should succeed"));
}

#[test]
fn saved_config_omits_compile_check_when_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    save_config(dir.path(), &papyrus_lints::Config::default())
        .expect("saving lint config should succeed");

    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config file");
    assert!(!contents.contains("compile_check"));
}
