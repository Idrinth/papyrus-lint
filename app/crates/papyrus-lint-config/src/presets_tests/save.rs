use super::*;

#[test]
fn save_user_preset_rejects_a_blank_name() {
    let error = save_user_preset_under(None, "   ", &papyrus_lints::Config::default(), false)
        .expect_err("blank name should be rejected");

    assert!(error.contains("must not be blank"));
}

#[test]
fn save_user_preset_rejects_built_in_preset_names_case_insensitively() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = save_user_preset_under(
        Some(base_dir.path()),
        "STRICT",
        &papyrus_lints::Config::default(),
        false,
    )
    .expect_err("built-in preset name should be rejected");

    assert!(error.contains("built-in preset name"));
    assert!(!base_dir.path().join(USER_PRESETS_DIR_NAME).exists());
}

#[test]
fn save_user_preset_errors_without_a_resolvable_base_dir() {
    let error = save_user_preset_under(None, "my-preset", &papyrus_lints::Config::default(), false)
        .expect_err("should fail without a base dir");

    assert!(error.contains("executable's directory"));
}

#[test]
fn save_user_preset_reports_an_error_when_the_presets_dir_cannot_be_created() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        base_dir.path().join(USER_PRESETS_DIR_NAME),
        "not a directory",
    )
    .expect("failed to create blocking file");

    let error = save_user_preset_under(
        Some(base_dir.path()),
        "my-preset",
        &papyrus_lints::Config::default(),
        false,
    )
    .expect_err("a file at the presets path should prevent saving");

    assert!(!error.is_empty());
}

#[test]
fn save_user_preset_creates_the_presets_directory_and_writes_the_file() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    let path = save_user_preset_under(Some(base_dir.path()), "my-preset", &config, false)
        .expect("saving a new preset should succeed");

    assert_eq!(
        path,
        base_dir
            .path()
            .join(USER_PRESETS_DIR_NAME)
            .join("my-preset.yaml")
    );
    let yaml = Preset::Custom("my-preset".to_string())
        .yaml(Some(base_dir.path()))
        .expect("saved preset should resolve");
    let saved: papyrus_lints::Config =
        serde_norway::from_str(&yaml).expect("saved preset should parse as a lint config");
    assert_eq!(saved, config);
}

#[test]
fn save_user_preset_trims_the_name_used_for_the_file() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let path = save_user_preset_under(
        Some(base_dir.path()),
        "  team-style  ",
        &papyrus_lints::Config::default(),
        false,
    )
    .expect("saving a trimmed preset name should succeed");

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("team-style.yaml")
    );
    assert_eq!(
        list_user_preset_names(&base_dir.path().join(USER_PRESETS_DIR_NAME)),
        vec!["team-style".to_string()]
    );
}

#[test]
fn save_user_preset_refuses_to_overwrite_without_the_flag() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    save_user_preset_under(
        Some(base_dir.path()),
        "my-preset",
        &papyrus_lints::Config::default(),
        false,
    )
    .expect("first save should succeed");

    let error = save_user_preset_under(
        Some(base_dir.path()),
        "my-preset",
        &papyrus_lints::Config::default(),
        false,
    )
    .expect_err("saving over an existing preset should fail without overwrite");

    assert!(error.contains("already exists"));
}

#[test]
fn save_user_preset_overwrites_an_existing_preset_case_insensitively_when_allowed() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    save_user_preset_under(
        Some(base_dir.path()),
        "My-Preset",
        &papyrus_lints::Config::default(),
        false,
    )
    .expect("first save should succeed");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    let path = save_user_preset_under(Some(base_dir.path()), "my-preset", &config, true)
        .expect("overwrite should succeed");

    // The differently-cased existing file is reused rather than a second
    // one being created alongside it.
    assert_eq!(
        path,
        base_dir
            .path()
            .join(USER_PRESETS_DIR_NAME)
            .join("My-Preset.yaml")
    );
    let entries: Vec<_> = fs::read_dir(base_dir.path().join(USER_PRESETS_DIR_NAME))
        .expect("failed to read presets dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(entries.len(), 1);
    let saved: papyrus_lints::Config = serde_norway::from_str(
        &Preset::Custom("my-preset".to_string())
            .yaml(Some(base_dir.path()))
            .expect("saved preset should resolve"),
    )
    .expect("saved preset should parse as a lint config");
    assert_eq!(saved, config);
}
