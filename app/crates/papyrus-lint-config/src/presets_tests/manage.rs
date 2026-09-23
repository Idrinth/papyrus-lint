use super::*;

#[test]
fn delete_user_preset_removes_the_matching_file_case_insensitively() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "My-Preset.yaml", "semicolon: true\n");

    delete_user_preset_under(Some(base_dir.path()), "my-preset")
        .expect("deleting an existing preset should succeed");

    assert!(list_user_preset_names(&presets_dir).is_empty());
}

#[test]
fn delete_user_preset_errors_for_an_unknown_preset() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = delete_user_preset_under(Some(base_dir.path()), "missing")
        .expect_err("deleting a missing preset should fail");

    assert!(error.contains("no preset named 'missing' exists"));
}

#[test]
fn delete_user_preset_errors_without_a_resolvable_base_dir() {
    let error =
        delete_user_preset_under(None, "my-preset").expect_err("should fail without a base dir");

    assert!(error.contains("executable's directory"));
}

#[test]
fn rename_user_preset_renames_the_matching_file() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");

    let path = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", false)
        .expect("renaming should succeed");

    assert_eq!(path, presets_dir.join("new-name.yaml"));
    assert_eq!(
        list_user_preset_names(&presets_dir),
        vec!["new-name".to_string()]
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: true\n");
}

#[test]
fn rename_user_preset_preserves_the_original_files_extension() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "old-name.yml", "semicolon: true\n");

    let path = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", false)
        .expect("renaming should succeed");

    assert_eq!(path, presets_dir.join("new-name.yml"));
}

#[test]
fn rename_user_preset_is_a_no_op_when_only_case_differs() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "my-preset.yaml", "semicolon: true\n");

    let path = rename_user_preset_under(Some(base_dir.path()), "my-preset", "My-Preset", false)
        .expect("a case-only rename should succeed");

    assert_eq!(path, presets_dir.join("my-preset.yaml"));
}

#[test]
fn rename_user_preset_errors_for_an_unknown_source_preset() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = rename_user_preset_under(Some(base_dir.path()), "missing", "new-name", false)
        .expect_err("renaming a missing preset should fail");

    assert!(error.contains("no preset named 'missing' exists"));
}

#[test]
fn rename_user_preset_errors_without_a_resolvable_base_dir() {
    let error = rename_user_preset_under(None, "old-name", "new-name", false)
        .expect_err("should fail without a base dir");

    assert!(error.contains("executable's directory"));
}

#[test]
fn rename_user_preset_rejects_a_blank_new_name() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");

    let error = rename_user_preset_under(Some(base_dir.path()), "old-name", "   ", false)
        .expect_err("blank new name should be rejected");

    assert!(error.contains("must not be blank"));
}

#[test]
fn rename_user_preset_rejects_a_built_in_preset_name_case_insensitively() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");

    let error = rename_user_preset_under(Some(base_dir.path()), "old-name", "STRICT", false)
        .expect_err("built-in preset name should be rejected");

    assert!(error.contains("built-in preset name"));
}

#[test]
fn rename_user_preset_refuses_to_overwrite_without_the_flag() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");
    write_config(&presets_dir, "new-name.yaml", "semicolon: false\n");

    let error = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", false)
        .expect_err("renaming over an existing preset should fail without overwrite");

    assert!(error.contains("already exists"));
}

#[test]
fn rename_user_preset_overwrites_an_existing_preset_when_allowed() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");
    write_config(&presets_dir, "new-name.yaml", "semicolon: false\n");

    let path = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", true)
        .expect("overwrite should succeed");

    assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: true\n");
    assert_eq!(
        list_user_preset_names(&presets_dir),
        vec!["new-name".to_string()]
    );
}

#[test]
fn read_user_preset_yaml_returns_the_files_raw_contents() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(&presets_dir, "my-preset.yaml", "semicolon: true\n");

    let yaml = read_user_preset_yaml_under(Some(base_dir.path()), "my-preset")
        .expect("reading an existing preset should succeed");

    assert_eq!(yaml, "semicolon: true\n");
}

#[test]
fn read_user_preset_yaml_errors_for_an_unknown_preset() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = read_user_preset_yaml_under(Some(base_dir.path()), "missing")
        .expect_err("reading a missing preset should fail");

    assert!(error.contains("unknown preset 'missing'"));
}

#[test]
fn list_user_preset_names_lists_yaml_and_yml_stems_sorted_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "Zebra.yaml", "");
    write_config(dir.path(), "abc.yml", "");
    write_config(dir.path(), "not-a-preset.txt", "");

    assert_eq!(
        list_user_preset_names(dir.path()),
        vec!["abc".to_string(), "Zebra".to_string()]
    );
}

#[test]
fn list_user_preset_names_returns_empty_for_a_missing_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        list_user_preset_names(&dir.path().join("does-not-exist")),
        Vec::<String>::new()
    );
}

#[test]
fn list_user_preset_names_ignores_yaml_directories() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "usable.YAML", "semicolon: true\n");
    fs::create_dir(dir.path().join("misleading.yml"))
        .expect("failed to create misleading directory");

    assert_eq!(
        list_user_preset_names(dir.path()),
        vec!["usable".to_string()]
    );
}
