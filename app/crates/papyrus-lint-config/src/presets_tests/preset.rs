use super::*;

#[test]
fn add_preset_errors_have_actionable_display_messages() {
    let invalid = AddPresetError::InvalidName("strict".to_string()).to_string();
    assert!(invalid.contains("'strict' can't be used as a preset name"));
    assert!(PRESET_NAMES.iter().all(|name| invalid.contains(name)));

    assert_eq!(
        AddPresetError::AlreadyExists(PathBuf::from("presets/custom.yaml")).to_string(),
        "a preset already exists at presets/custom.yaml"
    );
    assert_eq!(
        AddPresetError::BaseDirUnavailable.to_string(),
        "could not determine the running executable's directory"
    );
    assert_eq!(
        AddPresetError::Io("permission denied".to_string()).to_string(),
        "permission denied"
    );
}

#[test]
fn default_preset_is_strict() {
    assert_eq!(Preset::default(), Preset::Strict);
}

#[test]
fn preset_parse_matches_built_ins_case_insensitively_and_rejects_only_blank_names() {
    assert_eq!(Preset::parse("strict"), Some(Preset::Strict));
    assert_eq!(Preset::parse("STANDARD"), Some(Preset::Standard));
    assert_eq!(Preset::parse("Careful"), Some(Preset::Careful));
    assert_eq!(Preset::parse("   "), None);
    assert_eq!(Preset::parse(""), None);
}

#[test]
fn preset_parse_treats_any_other_name_as_a_custom_preset() {
    assert_eq!(
        Preset::parse("lenient"),
        Some(Preset::Custom("lenient".to_string()))
    );
    assert_eq!(
        Preset::parse("  my-preset  "),
        Some(Preset::Custom("my-preset".to_string()))
    );
}

#[test]
fn every_built_in_preset_yaml_parses_into_a_project_file() {
    for preset in [Preset::Strict, Preset::Standard, Preset::Careful] {
        let yaml = preset.yaml(None).expect("built-in preset should resolve");
        serde_norway::from_str::<ProjectFile>(&yaml)
            .unwrap_or_else(|err| panic!("{preset:?} preset failed to parse: {err}"));
    }
}

#[test]
fn custom_preset_yaml_errors_when_no_presets_dir_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = Preset::Custom("abc".to_string())
        .yaml(Some(dir.path()))
        .expect_err("should fail without a presets directory");

    assert!(error.contains("unknown preset 'abc'"));
}

#[test]
fn custom_preset_yaml_errors_when_no_matching_file_exists() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir(base_dir.path().join(USER_PRESETS_DIR_NAME))
        .expect("failed to create presets dir");

    let error = Preset::Custom("abc".to_string())
        .yaml(Some(base_dir.path()))
        .expect_err("should fail without a matching preset file");

    assert!(error.contains("unknown preset 'abc'"));
}

#[test]
fn custom_preset_yaml_reads_the_matching_file_case_insensitively() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "abc.yaml", "semicolon: true\n");

    let yaml = Preset::Custom("ABC".to_string())
        .yaml(Some(base_dir.path()))
        .expect("should find abc.yaml case-insensitively");

    assert_eq!(yaml.as_ref(), "semicolon: true\n");
}

#[test]
fn custom_preset_yaml_supports_uppercase_yml_extensions() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "team-style.YML", "semicolon: true\n");

    let yaml = Preset::Custom("team-style".to_string())
        .yaml(Some(base_dir.path()))
        .expect("uppercase YML preset should resolve");

    assert_eq!(yaml.as_ref(), "semicolon: true\n");
}

#[test]
fn custom_preset_yaml_reports_a_file_read_error() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    fs::write(presets_dir.join("invalid.yaml"), [0xff])
        .expect("failed to write invalid UTF-8 preset");

    let error = Preset::Custom("invalid".to_string())
        .yaml(Some(base_dir.path()))
        .expect_err("invalid UTF-8 should fail to load");

    assert!(error.contains("stream did not contain valid UTF-8"));
}
