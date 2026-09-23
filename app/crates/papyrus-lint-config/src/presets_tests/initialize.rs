use super::*;

#[test]
fn init_generates_a_config_from_a_custom_preset_under_the_base_dir() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "abc.yaml", "semicolon: true\n");

    let path = initialize_config_with_base(
        dir.path(),
        Some(base_dir.path()),
        Preset::Custom("abc".to_string()),
    )
    .expect("init should succeed from a custom preset");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    assert!(generated.contains("semicolon: true\n"));
}

#[test]
fn init_matches_a_custom_preset_name_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "Team-Style.yml", "semicolon: true\n");

    let path = initialize_config_with_base(
        dir.path(),
        Some(base_dir.path()),
        Preset::Custom("team-style".to_string()),
    )
    .expect("init should resolve a differently cased custom preset name");

    assert_eq!(
        load_config_from_path(&path).expect("generated config should parse"),
        papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        }
    );
}

#[test]
fn invalid_custom_preset_does_not_leave_a_partial_project_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "broken.yaml", "semicolon: [not a bool\n");

    let error = initialize_config_with_base(
        dir.path(),
        Some(base_dir.path()),
        Preset::Custom("broken".to_string()),
    )
    .expect_err("invalid custom preset YAML should be rejected");

    assert!(!error.is_empty());
    assert_eq!(config_file_path(dir.path()), None);
}

#[test]
fn init_reports_an_error_for_an_unknown_custom_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = initialize_config_with_base(
        dir.path(),
        Some(base_dir.path()),
        Preset::Custom("does-not-exist".to_string()),
    )
    .expect_err("init should fail for an unresolvable custom preset");

    assert!(error.contains("unknown preset 'does-not-exist'"));
}

#[test]
fn init_with_no_base_config_matches_init_with_a_base_dir_that_has_none() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let path = initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
        .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    let default_dir = tempfile::tempdir().expect("failed to create temp dir");
    let default_path = initialize_default_config(default_dir.path(), Preset::default())
        .expect("init should succeed");
    let default_generated =
        fs::read_to_string(&default_path).expect("failed to read generated config");

    assert_eq!(generated, default_generated);
}

#[test]
fn init_merges_an_executable_adjacent_base_config_over_the_defaults() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        base_dir.path(),
        "papyrus-lint.yaml",
        "compiler_path: /opt/PapyrusCompiler.exe\nsemicolon: true\n",
    );

    let path = initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
        .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    // The base's own settings win...
    assert!(generated.contains("compiler_path: /opt/PapyrusCompiler.exe\n"));
    assert!(generated.contains("semicolon: true\n"));
    // ...while everything the base didn't set still falls back to the
    // built-in default.
    assert!(generated.contains("indentation: tab\n"));
    assert!(generated.contains("strict_achlist_scope: false\n"));
}

#[test]
fn init_ignores_an_empty_executable_adjacent_base_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(base_dir.path(), "papyrus-lint.yaml", "");

    let path = initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
        .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    let default_dir = tempfile::tempdir().expect("failed to create temp dir");
    let default_path = initialize_default_config(default_dir.path(), Preset::default())
        .expect("init should succeed");
    let default_generated =
        fs::read_to_string(&default_path).expect("failed to read generated config");

    assert_eq!(generated, default_generated);
}

#[test]
fn init_errors_on_an_invalid_executable_adjacent_base_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        base_dir.path(),
        "papyrus-lint.yaml",
        "semicolon: [not a bool\n",
    );

    assert!(
        initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default()).is_err()
    );
}

#[test]
fn init_errors_when_the_project_directory_does_not_exist() {
    let parent = tempfile::tempdir().expect("failed to create temp dir");
    let missing_dir = parent.path().join("missing-project");

    let error = initialize_config_with_base(&missing_dir, None, Preset::default())
        .expect_err("init should fail when its target directory does not exist");

    assert!(!error.is_empty());
    assert!(!missing_dir.join(CONFIG_FILE_NAMES[0]).exists());
}

#[test]
fn init_refuses_to_replace_either_supported_config_name() {
    for name in CONFIG_FILE_NAMES {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), name, "semicolon: true\n");

        let error = initialize_config_with_base(dir.path(), None, Preset::default())
            .expect_err("init should reject an existing config");

        assert!(error.contains(name));
        assert_eq!(
            fs::read_to_string(dir.path().join(name)).expect("failed to read existing config"),
            "semicolon: true\n"
        );
    }
}
