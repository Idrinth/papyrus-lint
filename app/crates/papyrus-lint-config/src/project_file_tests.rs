use super::*;
use papyrus_lints::config::Indentation;

use crate::fallout4::detected_fallout4_script_lookup_dirs;
use crate::presets::{initialize_default_config, Preset};
use crate::skyrim::detected_skyrim_script_lookup_dirs;

fn write_config(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("failed to write test config file");
}

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

#[test]
fn parse_lint_yaml_returns_defaults_for_empty_and_whitespace_only_documents() {
    assert_eq!(
        parse_lint_yaml("").expect("empty yaml should parse"),
        papyrus_lints::Config::default()
    );
    assert_eq!(
        parse_lint_yaml(" \t\n\r\n").expect("whitespace-only yaml should parse"),
        papyrus_lints::Config::default()
    );
}

#[test]
fn parse_lint_yaml_applies_omitted_keys_as_defaults() {
    let config = parse_lint_yaml("semicolon: true\nindentation: space\n")
        .expect("partial yaml should parse");

    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::Space);
    assert_eq!(config.indentation_width, 4);
    assert!(!config.fail_on_warning);
    assert!(config.rules.trailing_whitespace);
}

#[test]
fn parse_lint_yaml_ignores_app_level_keys() {
    let config = parse_lint_yaml(
        "compiler_path: C:\\Tools\\PapyrusCompiler.exe\ncompile_check: true\nsemicolon: true\n",
    )
    .expect("project yaml should parse as lint config");

    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::default());
}

#[test]
fn parse_lint_yaml_rejects_invalid_yaml() {
    assert!(parse_lint_yaml("semicolon: [not a bool\n").is_err());
    assert!(parse_lint_yaml("indentation: eight-spaces\n").is_err());
}

#[test]
fn lint_config_to_yaml_round_trips_through_parse_lint_yaml() {
    let config = papyrus_lints::Config {
        semicolon: true,
        indentation: Indentation::Space,
        indentation_width: 2,
        ..papyrus_lints::Config::default()
    };

    let yaml = lint_config_to_yaml(&config).expect("config should serialize");
    assert_eq!(
        parse_lint_yaml(&yaml).expect("serialized config should parse"),
        config
    );
}

#[test]
fn save_config_at_path_rejects_invalid_existing_yaml_without_overwriting_it() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    let invalid = "semicolon: [not a bool\n";
    fs::write(&path, invalid).expect("failed to write test config file");

    assert!(save_config_at_path(&path, &papyrus_lints::Config::default()).is_err());
    assert_eq!(
        fs::read_to_string(path).expect("failed to read test config file"),
        invalid
    );
}

#[test]
fn default_config_matches_the_checked_in_configuration_copy() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let path =
        initialize_default_config(dir.path(), Preset::default()).expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    let checked_in_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../configuration/papyrus-lint.default.yaml");
    let checked_in_copy = fs::read_to_string(&checked_in_path)
        .expect("failed to read configuration/papyrus-lint.default.yaml");

    let detected = detected_skyrim_script_lookup_dirs();
    if detected.is_empty() {
        assert_eq!(
            generated, checked_in_copy,
            "configuration/papyrus-lint.default.yaml is out of date; regenerate it with `PapyrusLinterCLI init`"
        );
    } else {
        for dir in &detected {
            assert!(
                generated.contains(dir),
                "init should fill lookup_script_roots with {dir}"
            );
        }
    }
}

#[test]
fn errors_on_invalid_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: [not a bool\n");

    let result = load_config(dir.path());

    assert!(result.is_err());
}

#[test]
fn save_creates_yaml_file_when_none_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        indentation: Indentation::Space,
        indentation_width: 2,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    assert!(dir.path().join("papyrus-lint.yaml").is_file());
    assert!(!dir.path().join("papyrus-lint.yml").exists());
    let loaded = load_config(dir.path()).expect("loading should succeed");
    assert_eq!(loaded, config);
}

#[test]
fn save_annotates_top_level_keys_with_explanatory_comments() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config file");
    assert!(contents.starts_with("# Target game. Currently supported: skyrim, fallout4\ngame: skyrim\n"));
    assert!(contents.contains("# true, false\nsemicolon: true\n"));
    assert!(contents.contains("# tab, space\nindentation: tab\n"));
    assert!(contents.contains("# Each rule accepts true or false\nrules:\n"));
    // Nested rule keys aren't individually commented, matching the
    // README's example, which only comments the `rules:` block itself.
    assert!(!contents.contains("trailing_whitespace:\n  #"));
}

#[test]
fn save_overwrites_existing_yaml_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: false\n");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    let loaded = load_config(dir.path()).expect("loading should succeed");
    assert_eq!(loaded, config);
}

#[test]
fn save_prefers_existing_yml_file_over_creating_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    assert!(!dir.path().join("papyrus-lint.yaml").exists());
    let loaded = load_config(dir.path()).expect("loading should succeed");
    assert_eq!(loaded, config);
}

#[test]
fn load_compiler_path_returns_none_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        None
    );
}

#[test]
fn load_compiler_path_reads_explicit_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "compiler_path: C:\\Tools\\PapyrusCompiler.exe\n",
    );

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
}

#[test]
fn load_compiler_path_treats_blank_override_as_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "compiler_path: \"   \"\n");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        None
    );
}

#[test]
fn save_compiler_path_trims_the_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    save_compiler_path(dir.path(), Some("  C:\\Tools\\PapyrusCompiler.exe  "))
        .expect("saving compiler path should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
}

#[test]
fn save_compiler_path_persists_override_without_disturbing_lint_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
}

#[test]
fn save_config_preserves_existing_compiler_path_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
    );
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
}

#[test]
fn save_compiler_path_none_clears_override() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
        .expect("saving compiler path should succeed");

    save_compiler_path(dir.path(), None).expect("clearing compiler path should succeed");

    assert_eq!(
        load_compiler_path(dir.path()).expect("should succeed"),
        None
    );
}

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

#[test]
fn load_script_roots_returns_empty_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
}

#[test]
fn save_and_load_script_roots_round_trips_without_disturbing_lint_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    save_script_roots(
        dir.path(),
        &[
            "../SharedScripts".to_string(),
            "/abs/OtherScripts".to_string(),
        ],
    )
    .expect("saving script roots should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec![
            "../SharedScripts".to_string(),
            "/abs/OtherScripts".to_string()
        ]
    );
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
}

#[test]
fn save_script_roots_drops_blank_entries() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    save_script_roots(
        dir.path(),
        &[
            "  ".to_string(),
            "../SharedScripts".to_string(),
            String::new(),
        ],
    )
    .expect("saving script roots should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn load_script_roots_trims_entries_from_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "additional_script_roots:\n  - '  ../SharedScripts  '\n  - '   '\n",
    );

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn save_script_roots_empty_clears_existing_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    save_script_roots(dir.path(), &[]).expect("clearing script roots should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
}

#[test]
fn save_config_preserves_existing_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn load_lookup_script_roots_returns_empty_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
}

#[test]
fn save_and_load_lookup_script_roots_round_trips_without_disturbing_other_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    save_lookup_script_roots(
        dir.path(),
        &[
            "  C:/Skyrim/Data/Scripts/Source  ".to_string(),
            "  ".to_string(),
            "C:/Skyrim/Data/Source/Scripts".to_string(),
        ],
    )
    .expect("saving lookup roots should succeed");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        vec![
            "C:/Skyrim/Data/Scripts/Source".to_string(),
            "C:/Skyrim/Data/Source/Scripts".to_string()
        ]
    );
    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn save_lookup_script_roots_empty_is_kept_explicit_and_not_refilled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_lookup_script_roots(dir.path(), &["C:/Skyrim/Data/Scripts/Source".to_string()])
        .expect("saving lookup roots should succeed");

    save_lookup_script_roots(dir.path(), &[]).expect("clearing lookup roots should succeed");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config");
    assert!(contents.contains("lookup_script_roots:"));
}

#[test]
fn load_lookup_script_roots_trims_entries_from_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "lookup_script_roots:\n  - '  C:/Skyrim/Data/Scripts/Source  '\n  - '   '\n",
    );

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
    );
}

#[test]
fn save_config_preserves_existing_lookup_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_lookup_script_roots(dir.path(), &["C:/Skyrim/Data/Scripts/Source".to_string()])
        .expect("saving lookup roots should succeed");

    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
    );
}

#[test]
fn seed_lookup_script_roots_fills_only_when_the_key_was_missing() {
    let mut unset = ProjectFile::default();
    merge_lookup_roots(
        &mut unset.lookup_script_roots,
        &["C:/Skyrim/Data/Scripts/Source".to_string()],
    );
    assert_eq!(
        unset.lookup_script_roots,
        vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
    );

    let mut explicit = ProjectFile {
        lookup_script_roots_explicit: true,
        ..ProjectFile::default()
    };
    seed_lookup_script_roots(&mut explicit);
    assert!(explicit.lookup_script_roots.is_empty());
}

#[test]
fn project_file_from_yaml_seeds_lookup_script_roots_from_the_configured_game_not_skyrim() {
    let project = project_file_from_yaml("game: fallout4\n").expect("parsing should succeed");

    assert_eq!(
        project.lookup_script_roots,
        detected_fallout4_script_lookup_dirs(),
        "a fallout4 project must be seeded from Fallout 4's own detected install, \
         never Skyrim's (see issue #1187)"
    );
}

#[test]
fn seed_lookup_script_roots_uses_the_projects_configured_game() {
    let mut fallout4 = ProjectFile {
        lint: papyrus_lints::Config {
            game: papyrus_lints::Game::Fallout4,
            ..papyrus_lints::Config::default()
        },
        ..ProjectFile::default()
    };

    seed_lookup_script_roots(&mut fallout4);

    assert_eq!(
        fallout4.lookup_script_roots,
        detected_fallout4_script_lookup_dirs()
    );
}

#[test]
fn updating_a_config_without_lookup_script_roots_writes_the_key() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");

    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config");
    assert!(contents.contains("lookup_script_roots:"));
    assert!(contents.contains("semicolon: true"));
}
