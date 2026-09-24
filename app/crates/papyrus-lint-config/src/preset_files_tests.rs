use super::*;

#[test]
fn user_presets_dir_under_requires_an_existing_directory() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(user_presets_dir_under(None), None);
    assert_eq!(user_presets_dir_under(Some(base_dir.path())), None);

    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    std::fs::create_dir(&presets_dir).expect("failed to create presets dir");

    assert_eq!(
        user_presets_dir_under(Some(base_dir.path())),
        Some(presets_dir)
    );
}

#[test]
fn yaml_extension_matching_is_case_insensitive_and_rejects_other_paths() {
    assert!(has_yaml_extension(Path::new("preset.yaml")));
    assert!(has_yaml_extension(Path::new("preset.YML")));
    assert!(!has_yaml_extension(Path::new("preset.json")));
    assert!(!has_yaml_extension(Path::new("preset")));
}
