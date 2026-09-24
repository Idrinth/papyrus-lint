use super::*;
use crate::save_config;

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

#[test]
fn load_strict_achlist_scope_defaults_to_false_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert!(!load_strict_achlist_scope(dir.path()).expect("should succeed"));
}

#[test]
fn load_strict_achlist_scope_reads_the_configured_value() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "strict_achlist_scope: true\n",
    );

    assert!(load_strict_achlist_scope(dir.path()).expect("should succeed"));
}

#[test]
fn load_strict_achlist_scope_from_path_reads_an_explicit_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "strict_achlist_scope: true\n").expect("failed to write test config file");

    assert!(load_strict_achlist_scope_from_path(&path).expect("loading should succeed"));
}

#[test]
fn load_strict_achlist_scope_from_path_defaults_to_false_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "semicolon: true\n").expect("failed to write test config file");

    assert!(!load_strict_achlist_scope_from_path(&path).expect("loading should succeed"));
}

#[test]
fn load_strict_achlist_scope_from_path_returns_false_for_an_empty_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "").expect("failed to write test config file");

    assert!(!load_strict_achlist_scope_from_path(&path).expect("loading should succeed"));
}

#[test]
fn load_strict_achlist_scope_from_path_errors_when_the_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("missing-config.yaml");

    assert!(load_strict_achlist_scope_from_path(&path).is_err());
}

#[test]
fn load_strict_achlist_scope_from_path_errors_on_invalid_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "strict_achlist_scope: [not a bool\n")
        .expect("failed to write test config file");

    assert!(load_strict_achlist_scope_from_path(&path).is_err());
}

#[test]
fn saved_config_omits_strict_achlist_scope_when_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    save_config(dir.path(), &papyrus_lints::Config::default())
        .expect("saving lint config should succeed");

    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config file");
    assert!(!contents.contains("strict_achlist_scope"));
}
