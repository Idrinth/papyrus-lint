use std::fs;

use super::*;
use crate::{config_file_path, load_config_from_path};

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

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
fn user_presets_dir_under_requires_an_existing_directory() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(user_presets_dir_under(None), None);
    assert_eq!(user_presets_dir_under(Some(base_dir.path())), None);

    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");

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

#[test]
fn deep_merge_recurses_through_mappings_and_replaces_scalar_values() {
    let base = serde_norway::from_str(
        "semicolon: false\nrules:\n  property_sorting: false\n  trailing_whitespace: true\n",
    )
    .expect("base YAML should parse");
    let over = serde_norway::from_str(
        "semicolon: true\nrules:\n  property_sorting: true\nnew_setting: value\n",
    )
    .expect("override YAML should parse");

    let merged = deep_merge(base, over);
    let expected: serde_norway::Value = serde_norway::from_str(
        "semicolon: true\nrules:\n  property_sorting: true\n  trailing_whitespace: true\nnew_setting: value\n",
    )
    .expect("expected YAML should parse");

    assert_eq!(merged, expected);
}

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
fn add_user_preset_errors_without_a_base_dir() {
    let source = tempfile::tempdir()
        .expect("failed to create temp dir")
        .path()
        .join("source.yaml");

    let error = add_user_preset_under(None, "my-team", &source, false)
        .expect_err("a missing base dir should be reported as an error");

    assert_eq!(error, AddPresetError::BaseDirUnavailable);
}

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

#[test]
fn standard_preset_turns_off_purely_stylistic_rules_but_keeps_formatting() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let path = initialize_config_with_base(dir.path(), None, Preset::Standard)
        .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    assert!(generated.contains("  identifier_casing: false\n"));
    assert!(generated.contains("  trailing_whitespace: true\n"));
}

#[test]
fn careful_preset_relaxes_complexity_thresholds_and_disables_formatting() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let path = initialize_config_with_base(dir.path(), None, Preset::Careful)
        .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
    assert!(generated.contains("cyclomatic_complexity_error: 40\n"));
    assert!(generated.contains("  trailing_whitespace: false\n"));
}

#[test]
fn strict_preset_matches_the_built_in_default() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let path =
        initialize_config_with_base(dir.path(), None, Preset::Strict).expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    let default_dir = tempfile::tempdir().expect("failed to create temp dir");
    let default_path = initialize_default_config(default_dir.path(), Preset::default())
        .expect("init should succeed");
    let default_generated =
        fs::read_to_string(&default_path).expect("failed to read generated config");

    assert_eq!(generated, default_generated);
}

#[test]
fn executable_adjacent_base_config_overrides_a_non_strict_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        base_dir.path(),
        "papyrus-lint.yaml",
        "semicolon: true\nrules:\n  property_sorting: true\n",
    );

    let path = initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::Careful)
        .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    // The base's own settings win, even over the preset's own values...
    assert!(generated.contains("semicolon: true\n"));
    assert!(generated.contains("  property_sorting: true\n"));
    // ...while every other rule/setting still falls back to the
    // selected preset rather than the hardcoded built-in default.
    assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
    assert!(generated.contains("  trailing_whitespace: false\n"));
}

#[test]
fn preset_lint_config_matches_the_lint_settings_a_fresh_init_would_write() {
    let config = preset_lint_config(None, Preset::Careful).expect("should resolve");

    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = initialize_config_with_base(dir.path(), None, Preset::Careful)
        .expect("init should succeed");
    let initialized = load_config_from_path(&path).expect("failed to load generated config");

    assert_eq!(config, initialized);
}

#[test]
fn preset_lint_config_ignores_a_pre_existing_project_config() {
    // Unlike initialize_config_with_base, preset_lint_config never looks
    // at (or requires the absence of) a project's own config file: it
    // only resolves the preset's own settings, for resetting an
    // existing project's already-edited settings back to it in place.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");

    let config = preset_lint_config(None, Preset::Strict).expect("should resolve");

    assert_eq!(config, papyrus_lints::Config::default());
}

#[test]
fn preset_lint_config_is_still_layered_over_an_executable_adjacent_base_config() {
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        base_dir.path(),
        "papyrus-lint.yaml",
        "semicolon: true\nrules:\n  property_sorting: true\n",
    );

    let config =
        preset_lint_config(Some(base_dir.path()), Preset::Careful).expect("should resolve");

    assert!(config.semicolon);
    assert!(config.rules.property_sorting);
    // Every other rule/setting still falls back to the selected preset
    // rather than the hardcoded built-in default.
    assert_eq!(config.cyclomatic_complexity_warning, 20);
    assert!(!config.rules.trailing_whitespace);
}

#[test]
fn preset_lint_config_default_resolves_a_built_in_preset() {
    assert_eq!(
        preset_lint_config_default(Preset::Strict).expect("should resolve"),
        papyrus_lints::Config::default()
    );
}

#[test]
fn preset_lint_config_errors_on_an_unknown_custom_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = preset_lint_config(Some(dir.path()), Preset::Custom("missing".to_string()))
        .expect_err("should error");

    assert!(error.contains("unknown preset 'missing'"));
}
