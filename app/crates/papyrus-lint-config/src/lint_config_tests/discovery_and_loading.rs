use super::*;

#[test]
fn returns_defaults_when_no_config_file_present() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let config = load_config(dir.path()).expect("loading should succeed");

    assert_eq!(config, papyrus_lints::Config::default());
}

#[test]
fn loads_yaml_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "semicolon: true\nindentation: space\n",
    );

    let config = load_config(dir.path()).expect("loading should succeed");

    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::Space);
}

#[test]
fn loads_yml_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yml", "semicolon: true\n");

    let config = load_config(dir.path()).expect("loading should succeed");

    assert!(config.semicolon);
}

#[test]
fn prefers_yaml_over_yml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");
    write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");

    let config = load_config(dir.path()).expect("loading should succeed");

    assert!(config.semicolon);
}

#[test]
fn config_file_path_reports_the_selected_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    assert_eq!(config_file_path(dir.path()), None);

    let yml = dir.path().join("papyrus-lint.yml");
    write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
    assert_eq!(config_file_path(dir.path()), Some(yml));

    let yaml = dir.path().join("papyrus-lint.yaml");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");
    assert_eq!(config_file_path(dir.path()), Some(yaml));
}

#[test]
fn config_file_path_ignores_directories_with_config_names() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to create misleading config directory");

    assert_eq!(config_file_path(dir.path()), None);
}

#[test]
fn whitespace_only_project_config_returns_defaults() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", " \t\n\r\n");

    assert_eq!(
        load_config(dir.path()).expect("loading should succeed"),
        papyrus_lints::Config::default()
    );
}

#[test]
fn load_config_from_path_reads_an_explicit_file_regardless_of_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "semicolon: true\nindentation: space\n")
        .expect("failed to write test config file");

    let config = load_config_from_path(&path).expect("loading should succeed");

    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::Space);
}

#[test]
fn load_config_from_path_returns_defaults_for_an_empty_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "").expect("failed to write test config file");

    let config = load_config_from_path(&path).expect("loading should succeed");

    assert_eq!(config, papyrus_lints::Config::default());
}

#[test]
fn load_config_from_path_errors_when_the_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("missing.yaml");

    assert!(load_config_from_path(&path).is_err());
}

#[test]
fn save_config_at_path_creates_the_file_when_it_does_not_exist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    let config = papyrus_lints::Config {
        semicolon: true,
        indentation: Indentation::Space,
        ..papyrus_lints::Config::default()
    };

    save_config_at_path(&path, &config).expect("saving should succeed");

    assert!(path.is_file());
    assert_eq!(
        load_config_from_path(&path).expect("loading should succeed"),
        config
    );
}

#[test]
fn save_config_at_path_preserves_other_settings_already_in_the_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(
        &path,
        "compiler_path: C:\\Tools\\PapyrusCompiler.exe\nsemicolon: false\n",
    )
    .expect("failed to write test config file");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config_at_path(&path, &config).expect("saving should succeed");

    assert_eq!(
        load_config_from_path(&path).expect("loading should succeed"),
        config
    );
    let contents = fs::read_to_string(&path).expect("failed to read saved config file");
    assert!(contents.contains("compiler_path: C:\\Tools\\PapyrusCompiler.exe"));
}

#[test]
fn load_config_from_path_errors_on_invalid_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    fs::write(&path, "semicolon: [not a bool\n").expect("failed to write test config file");

    assert!(load_config_from_path(&path).is_err());
}
