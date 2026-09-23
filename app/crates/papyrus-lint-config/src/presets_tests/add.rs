use super::*;

#[test]
fn add_user_preset_rejects_a_blank_name_through_the_public_api() {
    let error = add_user_preset("   ", Path::new("unused.yaml"), false)
        .expect_err("a blank name should be rejected");

    assert_eq!(error, AddPresetError::InvalidName("   ".to_string()));
}

#[test]
fn add_user_preset_creates_the_presets_dir_and_copies_the_source_file() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, "semicolon: true\n").expect("failed to write source file");

    let path = add_user_preset_under(Some(base_dir.path()), "my-team", &source, false)
        .expect("adding a new preset should succeed");

    assert_eq!(
        path,
        base_dir
            .path()
            .join(USER_PRESETS_DIR_NAME)
            .join("my-team.yaml")
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: true\n");
    assert_eq!(
        list_user_preset_names(&base_dir.path().join(USER_PRESETS_DIR_NAME)),
        vec!["my-team".to_string()]
    );
}

#[test]
fn add_user_preset_refuses_to_overwrite_an_existing_preset_without_confirmation() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "my-team.yaml", "semicolon: true\n");
    let existing_path = presets_dir.join("my-team.yaml");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, "semicolon: false\n").expect("failed to write source file");

    let error = add_user_preset_under(Some(base_dir.path()), "my-team", &source, false)
        .expect_err("should refuse to overwrite without confirmation");

    assert_eq!(error, AddPresetError::AlreadyExists(existing_path.clone()));
    assert_eq!(
        fs::read_to_string(&existing_path).unwrap(),
        "semicolon: true\n"
    );
}

#[test]
fn add_user_preset_overwrites_an_existing_preset_when_confirmed() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    // The existing file is a `.yml`, so overwriting should reuse that
    // same path/extension rather than also creating a `.yaml` file.
    write_config(&presets_dir, "my-team.yml", "semicolon: true\n");
    let existing_path = presets_dir.join("my-team.yml");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, "semicolon: false\n").expect("failed to write source file");

    let path = add_user_preset_under(Some(base_dir.path()), "my-team", &source, true)
        .expect("overwriting with confirmation should succeed");

    assert_eq!(path, existing_path);
    assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: false\n");
    assert_eq!(
        list_user_preset_names(&presets_dir),
        vec!["my-team".to_string()]
    );
}

#[test]
fn add_user_preset_rejects_a_blank_name() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, "semicolon: true\n").expect("failed to write source file");

    let error = add_user_preset_under(Some(base_dir.path()), "   ", &source, false)
        .expect_err("a blank name should be rejected");

    assert_eq!(error, AddPresetError::InvalidName("   ".to_string()));
}

#[test]
fn add_user_preset_rejects_a_name_matching_a_built_in_preset() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, "semicolon: true\n").expect("failed to write source file");

    let error = add_user_preset_under(Some(base_dir.path()), "STRICT", &source, false)
        .expect_err("a name matching a built-in preset should be rejected");

    assert_eq!(error, AddPresetError::InvalidName("STRICT".to_string()));
}

#[test]
fn add_user_preset_errors_when_the_source_file_does_not_exist() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = add_user_preset_under(
        Some(base_dir.path()),
        "my-team",
        &base_dir.path().join("does-not-exist.yaml"),
        false,
    )
    .expect_err("a missing source file should be reported as an error");

    assert!(matches!(error, AddPresetError::Io(_)));
}

#[test]
fn add_user_preset_reports_invalid_utf8_in_the_source_file() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, [0xff]).expect("failed to write invalid UTF-8 source");

    let error = add_user_preset_under(Some(base_dir.path()), "my-team", &source, false)
        .expect_err("invalid UTF-8 should fail to copy");

    assert!(
        matches!(error, AddPresetError::Io(message) if message.contains("stream did not contain valid UTF-8"))
    );
}

#[test]
fn add_user_preset_reports_an_error_when_the_presets_dir_cannot_be_created() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let source = base_dir.path().join("source.yaml");
    fs::write(&source, "semicolon: true\n").expect("failed to write source file");
    fs::write(
        base_dir.path().join(USER_PRESETS_DIR_NAME),
        "not a directory",
    )
    .expect("failed to create blocking file");

    let error = add_user_preset_under(Some(base_dir.path()), "my-team", &source, false)
        .expect_err("a file at the presets path should prevent adding");

    assert!(matches!(error, AddPresetError::Io(_)));
}

#[test]
fn add_user_preset_errors_without_a_base_dir() {
    let source = tempfile::tempdir()
        .expect("failed to create temp dir")
        .path()
        .join("source.yaml");

    let error = add_user_preset_under(None, "my-team", &source, false)
        .expect_err("a missing base dir should be reported as an error");

    assert_eq!(error, AddPresetError::BaseDirUnavailable);
}
